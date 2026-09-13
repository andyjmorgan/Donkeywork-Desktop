import { describe, expect, it } from 'vitest';
import { ConsoleInput, consoleButton, keyboardHid, type InputTransport } from './consoleInput';

function setup() {
  let now = 0;
  const sent: any[] = [];
  const messages: string[] = [];
  const transport: InputTransport = {
    readyState: 'open', bufferedAmount: 0,
    send(data) { sent.push(JSON.parse(data)); },
    close() { this.readyState = 'closed'; },
  };
  const input = new ConsoleInput(transport, { width: 1920, height: 1080 }, (_, message) => messages.push(message), () => now);
  const receive = (message: object) => input.receive(JSON.stringify(message));
  const ready = () => receive({ type: 'ready', protocol: 'dwconsole.input', version: '0.2.0', leaseMs: 1000, generation: 'one', width: 1920, height: 1080 });
  const ack = (sequence: number) => receive({ type: 'ack', sequence, accepted: true });
  const control = () => { input.acquire(); ready(); };
  return { input, transport, sent, messages, ready, ack, receive, control, time: (value: number) => { now = value; } };
}

describe('explicit console input ownership', () => {
  it('never auto-acquires or buffers input before ready', () => {
    const h = setup();
    expect(h.input.enqueue({ type: 'key', hid: 4, down: true })).toBe(false);
    h.input.tick(true);
    expect(h.sent).toEqual([]);
    h.input.acquire();
    h.input.acquire();
    expect(h.input.enqueue({ type: 'button', button: 1, down: true, x: 1, y: 2 })).toBe(false);
    h.ready();
    expect(h.sent).toEqual([{ type: 'acquire' }]);
    expect(h.input.state).toBe('controlling');
  });

  it('sends exact sequence/generation and only one event until ack', () => {
    const h = setup(); h.control();
    h.input.enqueue({ type: 'key', hid: 224, down: true });
    h.input.enqueue({ type: 'button', button: 1, down: true, x: 10, y: 20 });
    expect(h.sent).toHaveLength(2);
    expect(h.sent[1]).toEqual({ type: 'event', generation: 'one', sequence: 1, event: { type: 'key', hid: 224, down: true } });
    h.ack(1);
    expect(h.sent[2].sequence).toBe(2);
    expect(h.sent[2].event.type).toBe('button');
  });

  it('coalesces adjacent unsent hover but not across a key/button boundary', () => {
    const h = setup(); h.control();
    h.input.enqueue({ type: 'move', x: 0, y: 0 }, true);
    h.input.enqueue({ type: 'move', x: 1, y: 1 }, true);
    h.input.enqueue({ type: 'move', x: 2, y: 2 }, true);
    h.input.enqueue({ type: 'button', button: 1, down: true, x: 2, y: 2 });
    h.input.enqueue({ type: 'move', x: 3, y: 3 });
    h.input.enqueue({ type: 'move', x: 4, y: 4 });
    for (let i = 1; i <= 5; i++) h.ack(i);
    expect(h.sent.slice(1).map(message => message.event)).toEqual([
      { type: 'move', x: 0, y: 0 }, { type: 'move', x: 2, y: 2 },
      { type: 'button', button: 1, down: true, x: 2, y: 2 },
      { type: 'move', x: 3, y: 3 }, { type: 'move', x: 4, y: 4 },
    ]);
  });

  it('release discards unsent actions and waits for released before new acquisition', () => {
    const h = setup(); h.control();
    h.input.enqueue({ type: 'key', hid: 4, down: true });
    h.input.enqueue({ type: 'key', hid: 4, down: false });
    h.input.release(); h.input.acquire(); h.ack(1);
    expect(h.input.state).toBe('releasing');
    expect(h.sent.map(message => message.type)).toEqual(['acquire', 'event', 'release']);
    h.receive({ type: 'released' });
    h.input.acquire(); h.ready();
    h.input.enqueue({ type: 'key', hid: 5, down: true });
    expect(h.sent.at(-1).sequence).toBe(1);
  });

  it('late ready after abandoned acquire cannot restore ownership', () => {
    const h = setup(); h.input.acquire(); h.input.release(); h.ready();
    expect(h.input.state).toBe('releasing');
    expect(h.input.raster).toBeNull();
    h.receive({ type: 'released' });
    expect(h.input.state).toBe('idle');
  });

  it('renew is timer-driven every 250ms and stops when not active', () => {
    const h = setup(); h.control();
    h.time(249); h.input.tick(true); expect(h.sent).toHaveLength(1);
    h.time(250); h.input.tick(true); expect(h.sent[1].event).toEqual({ type: 'renew' });
    h.ack(1); h.time(500); h.input.tick(false);
    expect(h.sent.at(-1)).toEqual({ type: 'release' });
    h.time(750); h.input.tick(true); expect(h.sent).toHaveLength(3);
  });

  it('closes on stalled acknowledgement and never replays actions', () => {
    const h = setup(); h.control();
    h.input.enqueue({ type: 'key', hid: 4, down: true });
    h.input.enqueue({ type: 'key', hid: 4, down: false });
    h.time(500); h.input.tick(true); h.ack(1);
    expect(h.transport.readyState).toBe('closed');
    expect(h.input.state).toBe('unavailable');
    expect(h.sent).toHaveLength(2);
  });

  it('late ack fails even if timer was suspended', () => {
    const h = setup(); h.control(); h.input.enqueue({ type: 'renew' });
    h.time(501); h.ack(1);
    expect(h.transport.readyState).toBe('closed');
  });

  it('old queued action is rejected even if current ack is timely', () => {
    const h = setup(); h.control();
    h.input.enqueue({ type: 'move', x: 0, y: 0 }, true);
    h.input.enqueue({ type: 'move', x: 1, y: 1 }, true);
    h.input.enqueue({ type: 'key', hid: 4, down: true });
    h.time(400); h.ack(1);
    h.time(501); h.ack(2);
    expect(h.transport.readyState).toBe('closed');
    expect(h.sent.filter(message => message.event?.type === 'key')).toEqual([]);
  });

  it('coalescing does not refresh stale queue age', () => {
    const h = setup(); h.control(); h.input.enqueue({ type: 'renew' });
    h.input.enqueue({ type: 'move', x: 0, y: 0 }, true);
    h.input.enqueue({ type: 'key', hid: 4, down: true });
    h.time(400); h.ack(1);
    h.input.enqueue({ type: 'move', x: 1, y: 1 }, true);
    h.time(501); h.input.enqueue({ type: 'move', x: 2, y: 2 }, true);
    h.input.tick(true);
    expect(h.transport.readyState).toBe('closed');
  });

  it('bounds non-coalescible queue', () => {
    const h = setup(); h.control(); h.input.enqueue({ type: 'renew' });
    for (let i = 0; i < 65; i++) h.input.enqueue({ type: 'key', hid: 4, down: i % 2 === 0 });
    expect(h.transport.readyState).toBe('closed');
    expect(h.sent).toHaveLength(2);
  });

  it('rejects buffered transport congestion without queueing reset behind it', () => {
    const h = setup(); h.control(); h.transport.bufferedAmount = 16385;
    h.input.enqueue({ type: 'renew' });
    expect(h.transport.readyState).toBe('closed');
    expect(h.sent).toHaveLength(1);
  });

  it('handles send errors without leaking payload into status', () => {
    const h = setup(); h.control(); h.transport.send = () => { throw new Error('sensitive'); };
    expect(() => h.input.enqueue({ type: 'key', hid: 4, down: true })).not.toThrow();
    expect(h.messages.join(' ')).not.toContain('sensitive');
    expect(h.transport.readyState).toBe('closed');
  });

  it.each([NaN, 0, 1919])('rejects mismatched source width %s', width => {
    const h = setup(); h.input.acquire(); h.receive({ type: 'ready', protocol: 'dwconsole.input', version: '0.2.0', leaseMs: 1000, generation: 'one', width, height: 1080 });
    expect(h.transport.readyState).toBe('closed');
    expect(h.input.raster).toBeNull();
  });

  it.each(['bad json', 'null', '[]', 'x'.repeat(4097)])('rejects malformed response', data => {
    const h = setup(); h.control(); h.input.receive(data);
    expect(h.transport.readyState).toBe('closed');
  });

  it.each([{ type: 'ack', sequence: 2, accepted: true }, { type: 'ack', sequence: 1 }, { type: 'ack', sequence: 1, accepted: false }, { type: 'unknown' }])('rejects invalid acknowledgement %j', response => {
    const h = setup(); h.control(); h.input.enqueue({ type: 'renew' }); h.receive(response);
    expect(h.transport.readyState).toBe('closed');
  });

  it.each([{ protocol: 'other' }, { version: '0.1.0' }, { leaseMs: 2000 }, { leaseMs: undefined }])('rejects incompatible ready %j', fields => {
    const h = setup(); h.input.acquire();
    h.receive({ type: 'ready', protocol: 'dwconsole.input', version: '0.2.0', leaseMs: 1000, generation: 'one', width: 1920, height: 1080, ...fields });
    expect(h.transport.readyState).toBe('closed');
  });

  it('unavailable is view-only and requires another explicit acquire', () => {
    const h = setup(); h.input.acquire(); h.receive({ type: 'unavailable', reason: 'No input helper' });
    h.time(500); h.input.tick(true);
    expect(h.input.state).toBe('unavailable');
    expect(h.sent).toHaveLength(1);
    h.input.acquire(); expect(h.sent).toHaveLength(2);
  });

  it('acquire/release barriers have finite deadlines', () => {
    const h = setup(); h.input.acquire(); h.time(2000); h.input.tick(true);
    expect(h.transport.readyState).toBe('closed');
    const j = setup(); j.control(); j.input.release(); j.time(2000); j.input.tick(true);
    expect(j.transport.readyState).toBe('closed');
  });
});

describe('physical input mappings', () => {
  it('maps letters, digits and distinct left/right modifiers', () => {
    expect(keyboardHid('KeyA')).toBe(4); expect(keyboardHid('KeyZ')).toBe(29);
    expect(keyboardHid('Digit1')).toBe(30); expect(keyboardHid('Digit0')).toBe(39);
    expect(keyboardHid('ShiftLeft')).toBe(225); expect(keyboardHid('ShiftRight')).toBe(229);
    expect(keyboardHid('ArrowUp')).toBe(82); expect(keyboardHid('Escape')).toBe(41);
  });
  it.each(['a', 'é', 'Process', 'Unidentified', 'Power', '__proto__', 'toString', 'Numpad1'])('rejects unmapped code %s', code => {
    expect(keyboardHid(code)).toBeNull();
  });
  it('maps DOM middle/right correctly and rejects extra buttons', () => {
    expect([0, 1, 2, 3, 4, -1].map(consoleButton)).toEqual([1, 3, 2, null, null, null]);
  });
});
