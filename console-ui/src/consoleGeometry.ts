/** All bounds and client coordinates are CSS pixels, never device pixels. */
export interface ConsoleRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Full-display, post-rotation source raster, independent of encoder downsampling. */
export interface ConsoleRaster {
  width: number;
  height: number;
}

export interface ConsolePoint {
  x: number;
  y: number;
}

/**
 * Geometry for centered object-fit: contain, without borders, padding or CSS
 * transforms. The caller supplies the video content box, not its border box.
 * Encoding must preserve the source aspect ratio (square pixels, no crop).
 * null means unavailable/invalid geometry; input must then be suppressed.
 */
export function consoleContentRect(bounds: ConsoleRect, source: ConsoleRaster): ConsoleRect | null {
  if (!Number.isSafeInteger(source.width) || !Number.isSafeInteger(source.height)
      || source.width <= 0 || source.height <= 0
      || ![bounds.left, bounds.top, bounds.width, bounds.height].every(Number.isFinite)
      || bounds.width <= 0 || bounds.height <= 0) return null;

  const scale = Math.min(bounds.width / source.width, bounds.height / source.height);
  const width = source.width * scale;
  const height = source.height * scale;
  const left = bounds.left + (bounds.width - width) / 2;
  const top = bounds.top + (bounds.height - height) / 2;
  if (![left, top, width, height, left + width, top + height].every(Number.isFinite)
      || width <= 0 || height <= 0 || left + width <= left || top + height <= top) return null;
  return { left, top, width, height };
}

/**
 * Map a client point to integer source pixels. New presses use the default
 * reject policy: bars/outside/right-bottom exclusive edges are not clickable.
 * An already-held drag may use clamp, including for its final release outside.
 * This function does not manage pointer capture, topology or held-button state.
 */
export function consoleSourcePoint(
  clientX: number,
  clientY: number,
  bounds: ConsoleRect,
  source: ConsoleRaster,
  policy: 'reject' | 'clamp' = 'reject',
): ConsolePoint | null {
  if (!Number.isFinite(clientX) || !Number.isFinite(clientY)
      || (policy !== 'reject' && policy !== 'clamp')) return null;
  const content = consoleContentRect(bounds, source);
  if (!content) return null;
  const right = content.left + content.width;
  const bottom = content.top + content.height;
  if (policy === 'reject' && (clientX < content.left || clientX >= right
      || clientY < content.top || clientY >= bottom)) return null;

  const x = Math.min(right, Math.max(content.left, clientX));
  const y = Math.min(bottom, Math.max(content.top, clientY));
  return {
    x: Math.min(source.width - 1, Math.max(0, Math.floor((x - content.left) / content.width * source.width))),
    y: Math.min(source.height - 1, Math.max(0, Math.floor((y - content.top) / content.height * source.height))),
  };
}
