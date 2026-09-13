// UI-only fixture state. These are not transport messages or broker contracts.
export type Resolution = { width: number; height: number };
export const demoModes: readonly Resolution[] = [
  { width: 3840, height: 2160 },
  { width: 1920, height: 1080 },
];
export type ScaleMode = 'fit' | 'native';
export type DemoState = {
  open: boolean;
  mode: Resolution;
  pending: Resolution | null;
  revision: number;
  notice: string;
};
export const initialState: DemoState = {
  open: true,
  mode: demoModes[0],
  pending: null,
  revision: 1,
  notice: 'Local display preview. No connection to Spark.',
};
export type DemoAction =
  | { type: 'open' }
  | { type: 'close' }
  | { type: 'resize'; mode: Resolution }
  | { type: 'resize-applied' }
  | { type: 'resize-failed' };
export function sameMode(a: Resolution, b: Resolution): boolean {
  return a.width === b.width && a.height === b.height;
}
export function label(mode: Resolution): string {
  return `${mode.width} × ${mode.height}`;
}
export function demoReducer(state: DemoState, action: DemoAction): DemoState {
  switch (action.type) {
    case 'open': return { ...state, open: true, notice: 'Local display preview. No connection to Spark.' };
    case 'close': return { ...state, open: false, pending: null, notice: 'Preview closed. No remote session was opened.' };
    case 'resize': {
      if (!state.open || state.pending || !demoModes.some(mode => sameMode(mode, action.mode))) return state;
      if (sameMode(state.mode, action.mode)) return { ...state, notice: `Preview is already ${label(state.mode)}.` };
      return { ...state, pending: action.mode, notice: 'Simulating a remote resolution change…' };
    }
    case 'resize-applied': {
      if (!state.open || !state.pending) return state;
      return { ...state, mode: state.pending, pending: null, revision: state.revision + 1,
        notice: `Simulated resolution changed to ${label(state.pending)}. No host display was changed.` };
    }
    case 'resize-failed': {
      if (!state.open || !state.pending) return state;
      return { ...state, pending: null, notice: `Simulated change failed. Preview remains ${label(state.mode)}.` };
    }
  }
}

export function layout(source: Resolution, viewport: Resolution, mode: ScaleMode) {
  const scale = mode === 'native' ? 1 : Math.min(1, viewport.width / source.width, viewport.height / source.height);
  return { scale, width: source.width * scale, height: source.height * scale };
}

/** Preview coordinate math only. No network operation or input injection. */
export function displayPoint(
  clientX: number, clientY: number,
  bounds: { left: number; top: number; width: number; height: number },
  source: Resolution,
): { x: number; y: number } | null {
  if (![clientX, clientY, bounds.left, bounds.top, bounds.width, bounds.height, source.width, source.height].every(Number.isFinite)
    || bounds.width <= 0 || bounds.height <= 0 || source.width <= 0 || source.height <= 0) return null;
  const x = clientX - bounds.left;
  const y = clientY - bounds.top;
  if (x < 0 || y < 0 || x >= bounds.width || y >= bounds.height) return null;
  return { x: Math.floor(x / bounds.width * source.width), y: Math.floor(y / bounds.height * source.height) };
}
