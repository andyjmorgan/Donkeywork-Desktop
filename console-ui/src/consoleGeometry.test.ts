import { describe, expect, it } from 'vitest';
import { consoleContentRect, consoleSourcePoint } from './consoleGeometry';

const hd = { width: 1920, height: 1080 };
const bounds = { left: 100, top: 50, width: 960, height: 540 };

describe('console contain geometry', () => {
  it('maps corners and centre with exclusive right/bottom boundaries', () => {
    expect(consoleSourcePoint(100, 50, bounds, hd)).toEqual({ x: 0, y: 0 });
    expect(consoleSourcePoint(580, 320, bounds, hd)).toEqual({ x: 960, y: 540 });
    expect(consoleSourcePoint(1059.99, 589.99, bounds, hd)).toEqual({ x: 1919, y: 1079 });
    expect(consoleSourcePoint(1060, 320, bounds, hd)).toBeNull();
    expect(consoleSourcePoint(580, 590, bounds, hd)).toBeNull();
  });

  it('rejects new clicks in letterbox bars', () => {
    const box = { left: 0, top: 0, width: 960, height: 600 };
    expect(consoleContentRect(box, hd)).toEqual({ left: 0, top: 30, width: 960, height: 540 });
    expect(consoleSourcePoint(480, 29, box, hd)).toBeNull();
    expect(consoleSourcePoint(480, 570, box, hd)).toBeNull();
    expect(consoleSourcePoint(480, 300, box, hd)).toEqual({ x: 960, y: 540 });
  });

  it('rejects new clicks in pillarbox bars', () => {
    const box = { left: 10, top: 20, width: 1200, height: 540 };
    expect(consoleContentRect(box, hd)).toEqual({ left: 130, top: 20, width: 960, height: 540 });
    expect(consoleSourcePoint(129, 100, box, hd)).toBeNull();
    expect(consoleSourcePoint(1090, 100, box, hd)).toBeNull();
  });

  it('clamps an existing drag and release outside, never a default new press', () => {
    expect(consoleSourcePoint(-500, -500, bounds, hd, 'clamp')).toEqual({ x: 0, y: 0 });
    expect(consoleSourcePoint(9000, 9000, bounds, hd, 'clamp')).toEqual({ x: 1919, y: 1079 });
    expect(consoleSourcePoint(-500, -500, bounds, hd)).toBeNull();
  });

  it('uses CSS client coordinates through fractional scaling and scrolling', () => {
    const box = { left: -100.25, top: -40.75, width: 768, height: 432 };
    expect(consoleSourcePoint(283.75, 175.25, box, hd)).toEqual({ x: 960, y: 540 });
    // DPR is deliberately absent: both client coordinates and DOM bounds are CSS units.
    expect(consoleSourcePoint(283.75, 175.25, box, { width: 3840, height: 2160 }))
      .toEqual({ x: 1920, y: 1080 });
  });

  it('recomputes mapping after source-mode changes without retaining old geometry', () => {
    const box = { left: 0, top: 0, width: 960, height: 600 };
    expect(consoleSourcePoint(0, 0, box, hd)).toBeNull();
    expect(consoleSourcePoint(0, 0, box, { width: 1920, height: 1200 })).toEqual({ x: 0, y: 0 });
  });

  it('handles a one-pixel raster', () => {
    expect(consoleSourcePoint(580, 320, bounds, { width: 1, height: 1 })).toEqual({ x: 0, y: 0 });
  });

  it.each([0, -1, NaN, Infinity, -Infinity])('rejects invalid box dimensions: %s', value => {
    expect(consoleContentRect({ ...bounds, width: value }, hd)).toBeNull();
    expect(consoleContentRect({ ...bounds, height: value }, hd)).toBeNull();
  });

  it.each([0, -1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])('rejects invalid raster dimensions: %s', value => {
    expect(consoleContentRect(bounds, { ...hd, width: value })).toBeNull();
    expect(consoleContentRect(bounds, { ...hd, height: value })).toBeNull();
  });

  it.each([NaN, Infinity, -Infinity])('rejects non-finite points and origins: %s', value => {
    expect(consoleSourcePoint(value, 50, bounds, hd, 'clamp')).toBeNull();
    expect(consoleSourcePoint(100, value, bounds, hd, 'clamp')).toBeNull();
    expect(consoleContentRect({ ...bounds, left: value }, hd)).toBeNull();
    expect(consoleContentRect({ ...bounds, top: value }, hd)).toBeNull();
  });

  it('rejects numeric overflow/precision collapse rather than returning broken coordinates', () => {
    expect(consoleContentRect({ ...bounds, left: Number.MAX_VALUE }, hd)).toBeNull();
    expect(consoleContentRect({ ...bounds, width: Number.MIN_VALUE }, hd)).toBeNull();
  });
});
