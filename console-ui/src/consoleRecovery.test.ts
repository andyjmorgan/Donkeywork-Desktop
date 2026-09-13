import { describe, expect, it } from 'vitest';
import { applyStalledStatus, canOfferConsole, restartForInitialVideoTimeout } from './consoleRecovery';

describe('same-peer console recovery decisions', () => {
  it('permits offers while capture is recovering with its existing header', () => {
    expect(canOfferConsole('recovering')).toBe(true);
    expect(canOfferConsole('streaming')).toBe(true);
    expect(canOfferConsole('failed')).toBe(false);
    expect(canOfferConsole(undefined)).toBe(false);
  });
  it('does not restart a peer at startup deadline during known capture recovery', () => {
    expect(restartForInitialVideoTimeout('recovering', 0)).toBe(false);
    expect(restartForInitialVideoTimeout('streaming', 0)).toBe(true);
  });
  it('retains an established video peer even if capture says streaming but frames stall', () => {
    expect(restartForInitialVideoTimeout('recovering', 100)).toBe(false);
    expect(restartForInitialVideoTimeout('streaming', 100)).toBe(false);
  });
  it('applies status only while the same frame remains stalled for two ticks', () => {
    expect(applyStalledStatus(100, 100, 2)).toBe(true);
    expect(applyStalledStatus(100, 100, 1)).toBe(false);
    expect(applyStalledStatus(100, 101, 0)).toBe(false);
    expect(applyStalledStatus(100, 102, 2)).toBe(false);
  });
});
