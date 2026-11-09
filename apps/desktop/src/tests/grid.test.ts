import { describe, expect, it } from 'vitest';
import { columnsFor, reorder } from '../features/grid/layout';
import { nextTransport } from '../player/transport';

describe('grid math', () => {
  it('fits all cameras and respects explicit column choices', () => {
    expect(columnsFor(0, 'auto')).toBe(1);
    expect(columnsFor(1, 'auto')).toBe(1);
    expect(columnsFor(9, 'auto')).toBe(3);
    expect(columnsFor(25, 'auto')).toBe(5);
    expect(columnsFor(9, '2')).toBe(2);
  });
  it('moves one camera without losing IDs or mutating the input', () => {
    const ids = ['a', 'b', 'c'];
    expect(reorder(ids, 'a', 'c')).toEqual(['b', 'c', 'a']);
    expect(ids).toEqual(['a', 'b', 'c']);
    expect(reorder(ids, 'missing', 'c')).toEqual(ids);
  });
});
describe('transport fallback', () => {
  it('tries WebRTC, MSE, HLS then surfaces failure', () => {
    expect(nextTransport('webrtc')).toBe('mse');
    expect(nextTransport('mse')).toBe('hls');
    expect(nextTransport('hls')).toBeNull();
  });
});
