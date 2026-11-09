export class VideoRTC extends HTMLElement {
  mode: string;
  wsState: number;
  media: string;
  background: boolean;
  visibilityCheck: boolean;
  DISCONNECT_TIMEOUT: number;
  pcConfig: RTCConfiguration;
  video: HTMLVideoElement;
  src: string;
  oninit(): void;
  onclose(): void;
  ondisconnect(): void;
}

