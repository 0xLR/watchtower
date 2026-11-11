// WHEP keeps engine-specific signalling outside the player and UI.
export function connectWhep(video: HTMLVideoElement, url: string, failed: () => void): () => void {
  const peer = new RTCPeerConnection({ iceServers: [], bundlePolicy: 'max-bundle' });
  const abort = new AbortController();
  let disposed = false;
  let session: string | undefined;
  peer.addTransceiver('video', { direction: 'recvonly' });
  peer.ontrack = event => { if (!disposed) video.srcObject = event.streams[0] ?? new MediaStream([event.track]); };
  peer.onconnectionstatechange = () => { if (!disposed && ['failed', 'disconnected'].includes(peer.connectionState)) failed(); };
  void (async () => {
    try {
      await peer.setLocalDescription(await peer.createOffer());
      if (peer.iceGatheringState !== 'complete') {
        await new Promise<void>((resolve, reject) => {
          const timer = setTimeout(() => { cleanup(); reject(new Error('ICE gathering timed out')); }, 4000);
          const gathered = () => { if (peer.iceGatheringState === 'complete') { cleanup(); resolve(); } };
          const cancelled = () => { cleanup(); reject(new Error('Playback cancelled')); };
          const cleanup = () => { clearTimeout(timer); peer.removeEventListener('icegatheringstatechange', gathered); abort.signal.removeEventListener('abort', cancelled); };
          peer.addEventListener('icegatheringstatechange', gathered); abort.signal.addEventListener('abort', cancelled, { once: true }); gathered();
        });
      }
      if (disposed) return;
      const response = await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/sdp' }, body: peer.localDescription?.sdp, signal: abort.signal });
      if (!response.ok) throw new Error('WebRTC negotiation failed');
      const location = response.headers.get('Location');
      if (location) { const endpoint = new URL(location, url); if (endpoint.origin === new URL(url).origin) session = endpoint.href; }
      const answer = await response.text();
      if (!disposed) await peer.setRemoteDescription({ type: 'answer', sdp: answer });
    } catch { if (!disposed) failed(); }
  })();
  return () => {
    disposed = true; abort.abort(); peer.close(); video.pause(); video.srcObject = null;
    if (session) void fetch(session, { method: 'DELETE', keepalive: true }).catch(() => {});
    video.remove();
  };
}
