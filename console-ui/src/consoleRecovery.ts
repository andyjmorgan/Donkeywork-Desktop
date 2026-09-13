export type CaptureState = 'streaming' | 'recovering';

/** Recovering still has a valid stream header and can negotiate an existing peer. */
export function canOfferConsole(state: unknown): state is CaptureState {
  return state === 'streaming' || state === 'recovering';
}

/**
 * The startup deadline diagnoses missing first video, not capture handoffs.
 * Once video has arrived, a healthy peer must survive capture-only stalls.
 */
export function restartForInitialVideoTimeout(state: CaptureState, decodedFrames: number): boolean {
  return decodedFrames === 0 && state !== 'recovering';
}

/** Ignore a slow status request after newer frames have already restored output. */
export function applyStalledStatus(requestFrames: number, currentFrames: number, staleTicks: number): boolean {
  return requestFrames === currentFrames && staleTicks >= 2;
}
