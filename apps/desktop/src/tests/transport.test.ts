import { describe, expect, it } from 'vitest';
import { nextSupportedTransport } from '../player/transport';
describe('engine transport capabilities', () => {
  it('skips unsupported transports and stops at the engine boundary', () => {
    expect(nextSupportedTransport('webrtc', ['webrtc'])).toBeNull();
    expect(nextSupportedTransport('webrtc', ['webrtc', 'hls'])).toBe('hls');
    expect(nextSupportedTransport('webrtc', ['webrtc', 'mse', 'hls'])).toBe('mse');
    expect(nextSupportedTransport('hls', ['webrtc', 'hls'])).toBeNull();
  });
});
