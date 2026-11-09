export type Transport = 'webrtc' | 'mse' | 'hls';
export function nextTransport(current: Transport): Transport | null {
  return current === 'webrtc' ? 'mse' : current === 'mse' ? 'hls' : null;
}
