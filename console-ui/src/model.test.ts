import { describe, expect, it } from 'vitest';
import { demoModes, demoReducer, displayPoint, initialState, layout } from './model';

describe('explicit local preview state', () => {
  it('keeps the current display until the simulated resize is applied', () => {
    const pending = demoReducer(initialState, { type: 'resize', mode: demoModes[1] });
    expect(pending.mode).toEqual(demoModes[0]);
    const applied = demoReducer(pending, { type: 'resize-applied' });
    expect(applied.mode).toEqual(demoModes[1]);
    expect(applied.open).toBe(true);
    expect(applied.revision).toBe(2);
    const restored = demoReducer(demoReducer(applied, { type: 'resize', mode: demoModes[0] }), { type: 'resize-applied' });
    expect(restored.mode).toEqual(demoModes[0]);
    expect(restored.revision).toBe(3);
  });
  it('preserves the old mode on failure and ignores unsupported requests', () => {
    const failed = demoReducer(demoReducer(initialState, { type: 'resize', mode: demoModes[1] }), { type: 'resize-failed' });
    expect(failed.mode).toEqual(initialState.mode);
    expect(failed.revision).toBe(1);
    expect(failed.notice).toContain('failed');
    expect(demoReducer(initialState, { type: 'resize', mode: { width: 999, height: 999 } })).toEqual(initialState);
  });
  it('cancels a pending resize on close and ignores its late completion', () => {
    const closed = demoReducer(demoReducer(initialState, { type: 'resize', mode: demoModes[1] }), { type: 'close' });
    expect(demoReducer(closed, { type: 'resize-applied' })).toEqual(closed);
  });
  it('does not replace an in-flight request or increment for no-op modes', () => {
    const pending = demoReducer(initialState, { type: 'resize', mode: demoModes[1] });
    expect(demoReducer(pending, { type: 'resize', mode: demoModes[0] })).toEqual(pending);
    expect(demoReducer(initialState, { type: 'resize', mode: demoModes[0] }).revision).toBe(1);
  });
});

describe('native raster and viewport are independent', () => {
  it('fits 4K without changing the source or distorting its aspect ratio', () => {
    expect(layout(demoModes[0], { width: 960, height: 600 }, 'fit')).toEqual({ scale: 0.25, width: 960, height: 540 });
    expect(layout(demoModes[0], { width: 960, height: 600 }, 'native')).toEqual({ scale: 1, width: 3840, height: 2160 });
  });
  it('maps physical corners after viewport scaling and scrolling', () => {
    const bounds = { left: -200, top: -100, width: 960, height: 540 };
    expect(displayPoint(-200, -100, bounds, demoModes[0])).toEqual({ x: 0, y: 0 });
    expect(displayPoint(759.9, 439.9, bounds, demoModes[0])).toEqual({ x: 3839, y: 2159 });
    expect(displayPoint(760, 440, bounds, demoModes[0])).toBeNull();
  });
  it('rejects points outside the display and invalid geometry', () => {
    const bounds = { left: 20, top: 30, width: 960, height: 540 };
    expect(displayPoint(0, 0, bounds, demoModes[0])).toBeNull();
    expect(displayPoint(20, 30, { ...bounds, width: 0 }, demoModes[0])).toBeNull();
    expect(displayPoint(NaN, 30, bounds, demoModes[0])).toBeNull();
  });
});
