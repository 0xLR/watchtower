import { VideoRTC } from './vendor/video-rtc.js';
import type { Transport } from './transport';
class VideoStream extends VideoRTC {
  constructor() {
    super();
    this.media = 'video'; this.pcConfig = { iceServers: [], bundlePolicy: 'max-bundle' };
    this.visibilityCheck = false; this.DISCONNECT_TIMEOUT = 0;
  }
  oninit() { super.oninit(); this.video.muted = true; this.video.controls = false; }
  ondisconnect() {
    if (this.video?.src.startsWith('blob:')) URL.revokeObjectURL(this.video.src);
    super.ondisconnect();
  }
  // React owns retries and fallback, so the upstream reconnect loop is disabled.
  onclose() { if (this.wsState !== WebSocket.CLOSED) this.ondisconnect(); }
}
if (!customElements.get('video-stream')) customElements.define('video-stream', VideoStream);
export function createStream(mode: Transport): VideoStream {
  const element = document.createElement('video-stream') as VideoStream;
  element.mode = mode;
  return element;
}
export type StreamElement = VideoStream;
