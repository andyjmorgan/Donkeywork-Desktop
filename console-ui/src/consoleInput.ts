import type { ConsoleRaster } from './consoleGeometry';

export type ConsoleInputEvent =
  | { type: 'move'; x: number; y: number }
  | { type: 'button'; button: number; down: boolean; x: number; y: number }
  | { type: 'wheel'; vertical: number; horizontal: number; x: number; y: number }
  | { type: 'key'; hid: number; down: boolean }
  | { type: 'renew' | 'reset' };
export type InputState = 'idle' | 'acquiring' | 'controlling' | 'releasing' | 'unavailable';
export interface InputTransport {
  readyState: string;
  bufferedAmount: number;
  send(data: string): void;
  close(): void;
}
type Pending = { event: ConsoleInputEvent; at: number; coalesce: boolean };
const MAX_QUEUE = 64;
const MAX_AGE_MS = 500;
const MAX_BUFFERED_BYTES = 16384;

/** One executor lane. No action retries, no queued input before explicit acquisition. */
export class ConsoleInput {
  state: InputState = 'idle';
  raster: ConsoleRaster | null = null;
  private generation = '';
  private sequence = 0;
  private queue: Pending[] = [];
  private flight: { sequence: number; at: number } | null = null;
  private transitionAt = 0;
  private lastRenew = 0;

  constructor(
    private transport: InputTransport,
    private expected: ConsoleRaster,
    private changed: (state: InputState, message: string) => void,
    private now: () => number = () => performance.now(),
  ) {}

  private status(state: InputState, message: string) {
    this.state = state;
    this.changed(state, message);
  }

  private send(value: object): boolean {
    if (this.transport.readyState !== 'open' || this.transport.bufferedAmount > MAX_BUFFERED_BYTES) {
      this.fail('Input transport unavailable or congested. Video remains view only.');
      return false;
    }
    try { this.transport.send(JSON.stringify(value)); return true; }
    catch { this.fail('Input send failed. Video remains view only.'); return false; }
  }

  acquire() {
    if (!['idle', 'unavailable'].includes(this.state)) return;
    if (this.transport.readyState !== 'open') {
      this.status('unavailable', 'Input unavailable on this connection. Video remains view only.');
      return;
    }
    this.transitionAt = this.now();
    this.status('acquiring', 'Acquiring console control…');
    this.send({ type: 'acquire' });
  }

  receive(data: unknown) {
    if (typeof data !== 'string' || data.length > 4096) { this.fail('Invalid input response.'); return; }
    let reply: Record<string, unknown>;
    try { reply = JSON.parse(data); }
    catch { this.fail('Invalid input response.'); return; }
    if (!reply || typeof reply !== 'object' || Array.isArray(reply)) { this.fail('Invalid input response.'); return; }
    if ((this.flight && this.now() - this.flight.at >= MAX_AGE_MS)
        || (['acquiring', 'releasing'].includes(this.state) && this.now() - this.transitionAt >= 2000)) {
      this.fail('Late input response discarded; control released.'); return;
    }
    if (reply.type === 'released') {
      if (this.state === 'releasing') this.status('idle', 'Click the video to control · physical keyboard layout');
      return;
    }
    // Release is a barrier: late ready/ack cannot reactivate an abandoned acquisition.
    if (this.state === 'releasing') return;
    if (reply.type === 'unavailable') {
      this.clear();
      const reason = typeof reply.reason === 'string' ? reply.reason.slice(0, 160) : 'Input unavailable';
      this.status('unavailable', `${reason}. Video remains view only.`);
      return;
    }
    if (reply.type === 'ready' && this.state === 'acquiring') {
      if (reply.protocol !== 'dwconsole.input' || reply.version !== '0.2.0' || reply.leaseMs !== 1000
          || typeof reply.generation !== 'string' || !reply.generation || reply.generation.length > 128
          || reply.width !== this.expected.width || reply.height !== this.expected.height
          || !Number.isSafeInteger(reply.width) || !Number.isSafeInteger(reply.height)
          || this.expected.width <= 0 || this.expected.height <= 0) {
        this.fail('Input display does not match the visible console.'); return;
      }
      this.generation = reply.generation;
      this.raster = { ...this.expected };
      this.sequence = 0;
      this.lastRenew = this.now();
      this.status('controlling', 'Controlling console · Ctrl+Alt+Escape releases input');
      return;
    }
    if (reply.type === 'ack' && this.state === 'controlling') {
      if (!this.flight || reply.sequence !== this.flight.sequence || reply.accepted !== true) {
        this.fail('Input acknowledgement rejected or out of order.'); return;
      }
      this.flight = null;
      this.pump();
      return;
    }
    if (this.state === 'controlling' || this.state === 'acquiring') this.fail('Unexpected input response.');
  }

  enqueue(event: ConsoleInputEvent, coalesce = false): boolean {
    if (this.state !== 'controlling') return false;
    const now = this.now();
    const last = this.queue.at(-1);
    // Replace only adjacent unsent motion; never jump over a button/key boundary.
    // Retain the oldest timestamp so continuous motion cannot mask congestion.
    if (coalesce && event.type === 'move' && last?.coalesce && last.event.type === 'move') last.event = event;
    else if (this.queue.length >= MAX_QUEUE) { this.fail('Input queue overflow; control released.'); return false; }
    else this.queue.push({ event, at: now, coalesce });
    this.pump();
    return this.state === 'controlling';
  }

  private pump() {
    if (this.state !== 'controlling' || this.flight) return;
    const next = this.queue.shift();
    if (!next) return;
    if (this.now() - next.at >= MAX_AGE_MS) { this.fail('Stale input discarded; control released.'); return; }
    this.sequence += 1;
    this.flight = { sequence: this.sequence, at: this.now() };
    this.send({ type: 'event', generation: this.generation, sequence: this.sequence, event: next.event });
  }

  /** Called periodically, independently of DOM repeat/motion events. */
  tick(active: boolean) {
    const now = this.now();
    if (['acquiring', 'releasing'].includes(this.state) && now - this.transitionAt >= 2000) {
      this.fail('Input negotiation timed out. Video remains view only.'); return;
    }
    if (this.state !== 'controlling') return;
    if (!active) { this.release(); return; }
    if ((this.flight && now - this.flight.at >= MAX_AGE_MS)
        || (this.queue[0] && now - this.queue[0].at >= MAX_AGE_MS)) {
      this.fail('Input stalled; pending actions discarded.'); return;
    }
    if (now - this.lastRenew >= 250) {
      this.lastRenew = now;
      this.enqueue({ type: 'renew' });
    }
  }

  release() {
    if (!['controlling', 'acquiring'].includes(this.state)) return;
    this.clear();
    this.transitionAt = this.now();
    this.status('releasing', 'Releasing console control…');
    // Bridge release performs an ordered reset and closes the helper lease.
    this.send({ type: 'release' });
  }

  private clear() {
    this.queue = []; this.flight = null; this.raster = null; this.generation = '';
  }

  fail(message = 'Input connection ended. Video remains view only.') {
    this.clear();
    this.status('unavailable', message);
    // Close rather than enqueue a reset behind a congested/uncertain stream.
    // Daemon disconnect/deadline release is authoritative.
    if (this.transport.readyState !== 'closed' && this.transport.readyState !== 'closing') this.transport.close();
  }
}

/** Physical USB keyboard usages. Unsupported/IME text is never guessed. */
export function keyboardHid(code: string): number | null {
  if (/^Key[A-Z]$/.test(code)) return code.charCodeAt(3) - 65 + 4;
  if (/^Digit[1-9]$/.test(code)) return Number(code[5]) + 29;
  const keys: Record<string, number> = {
    Digit0: 39, Enter: 40, Escape: 41, Backspace: 42, Tab: 43, Space: 44,
    Minus: 45, Equal: 46, BracketLeft: 47, BracketRight: 48, Backslash: 49,
    Semicolon: 51, Quote: 52, Backquote: 53, Comma: 54, Period: 55, Slash: 56,
    CapsLock: 57, Insert: 73, Home: 74, PageUp: 75, Delete: 76, End: 77, PageDown: 78,
    ArrowRight: 79, ArrowLeft: 80, ArrowDown: 81, ArrowUp: 82,
    ControlLeft: 224, ShiftLeft: 225, AltLeft: 226, MetaLeft: 227,
    ControlRight: 228, ShiftRight: 229, AltRight: 230, MetaRight: 231,
  };
  return Object.prototype.hasOwnProperty.call(keys, code) ? keys[code] : null;
}

/** DOM left/middle/right -> daemon left/right/middle. */
export function consoleButton(button: number): number | null {
  return button === 0 ? 1 : button === 1 ? 3 : button === 2 ? 2 : null;
}
