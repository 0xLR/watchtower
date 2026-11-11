export type Transport = 'webrtc' | 'mse' | 'hls';
export function nextSupportedTransport(current: Transport, supported: string[]): Transport | null {
  const order: Transport[] = ['webrtc', 'mse', 'hls'];
  return order.slice(order.indexOf(current) + 1).find(mode => supported.includes(mode)) ?? null;
}
export function nextTransport(current: Transport): Transport | null {
  return current === 'webrtc' ? 'mse' : current === 'mse' ? 'hls' : null;
}
