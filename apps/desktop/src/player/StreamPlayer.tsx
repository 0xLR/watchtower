import { useEffect, useRef, useState } from 'react';
import type { Playback } from '../../../../packages/ipc-types/Playback';
import { nextSupportedTransport, type Transport } from './transport';
import { connectWhep } from './whep';

export type PlayerState = 'connecting' | 'live' | 'unreachable' | 'paused';
export function StreamPlayer({ load, paused, onState }: { load: () => Promise<Playback>; paused: boolean; onState?: (state: PlayerState) => void }) {
  const host = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<PlayerState>('connecting');
  const [transport, setTransport] = useState<Transport>('webrtc');
  const [attempt, setAttempt] = useState(0);
  const [dimensions, setDimensions] = useState('');
  const stateCallback = useRef(onState); stateCallback.current = onState;
  useEffect(() => {
    const update = (value: PlayerState) => { setState(value); stateCallback.current?.(value); };
    if (paused) { update('paused'); return; }
    let disposed = false;
    let cleanup = () => {};
    let timeout: ReturnType<typeof setTimeout> | undefined;
    let progress: ReturnType<typeof setInterval> | undefined;
    let supported = ['webrtc', 'mse', 'hls'];
    update('connecting'); setDimensions('');
    const failed = () => {
      if (disposed) return;
      disposed = true; cleanup(); clearTimeout(timeout); clearInterval(progress);
      const next = nextSupportedTransport(transport, supported);
      if (next) setTransport(next);
      else { update('unreachable'); timeout = setTimeout(() => { setTransport('webrtc'); setAttempt(a => a + 1); }, 15000); }
    };
    void (async () => {
      try {
        const playback = await load();
        if (disposed) return;
        supported = playback.transports ?? supported;
        if (!supported.includes(transport)) {
          const first = supported.find(mode => ['webrtc', 'mse', 'hls'].includes(mode));
          if (first) { setTransport(first as Transport); return; }
          throw new Error('No supported playback transport');
        }
        let video: HTMLVideoElement;
        if (transport === 'hls') {
          video = document.createElement('video');
          video.muted = true; video.autoplay = true; video.playsInline = true;
          host.current?.appendChild(video);
          const { default: Hls } = await import('hls.js');
          if (disposed) { video.remove(); return; }
          if (Hls.isSupported()) {
            const hls = new Hls({ lowLatencyMode: true, maxBufferLength: 8 });
            cleanup = () => { hls.destroy(); video.remove(); };
            hls.on(Hls.Events.ERROR, (_event, data) => { if (data.fatal) failed(); });
            hls.on(Hls.Events.MANIFEST_PARSED, () => { void video.play().catch(() => {}); });
            hls.attachMedia(video); hls.loadSource(playback.hls_url);
          } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
            cleanup = () => { video.pause(); video.removeAttribute('src'); video.load(); video.remove(); };
            video.src = playback.hls_url;
          } else { video.remove(); failed(); return; }
        } else if (playback.whep_url && transport === 'webrtc') {
          video = document.createElement('video');
          video.muted = true; video.autoplay = true; video.playsInline = true;
          host.current?.appendChild(video);
          cleanup = connectWhep(video, playback.whep_url, failed);
        } else {
          const { createStream } = await import('./element');
          if (disposed) return;
          const element = createStream(transport);
          host.current?.appendChild(element);
          video = element.video;
          cleanup = () => { element.remove(); element.ondisconnect(); };
          element.src = playback.websocket_url;
        }
        video.addEventListener('error', failed);
        video.addEventListener('playing', () => {
          if (disposed) return;
          clearTimeout(timeout); update('live');
          setDimensions(`${video.videoWidth} × ${video.videoHeight}`);
        });
        let lastTime = -1, stalled = 0;
        progress = setInterval(() => {
          if (video.readyState >= 2 && video.currentTime === lastTime) stalled += 1; else stalled = 0;
          lastTime = video.currentTime;
          if (stalled >= 12) failed();
        }, 1000);
        timeout = setTimeout(failed, transport === 'hls' ? 16000 : playback.whep_url ? 15000 : 8000);
        void video.play().catch(() => {});
      } catch { failed(); }
    })();
    return () => { disposed = true; cleanup(); clearTimeout(timeout); clearInterval(progress); stateCallback.current?.('paused'); };
  }, [load, paused, transport, attempt]);
  return <div className="player-wrap">
    <div ref={host} className="player" />
    {state !== 'live' && <div className="player-state" role="status"><span className="status-mark" />{state === 'paused' ? 'Paused while window is hidden' : state === 'connecting' ? `Connecting / ${transport.toUpperCase()}` : 'Camera unreachable. Check its power and connection.'}{state === 'unreachable' && <button className="button" onClick={e => { e.stopPropagation(); setTransport('webrtc'); setAttempt(a => a + 1); }}>Retry now</button>}</div>}
    {state === 'live' && <span className="video-readout">{dimensions} / {transport.toUpperCase()}</span>}
  </div>;
}
