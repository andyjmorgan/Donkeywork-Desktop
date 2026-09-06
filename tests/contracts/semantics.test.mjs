import test from 'node:test';
import assert from 'node:assert/strict';
import { AttachmentStreamModel, DesktopModel, GrantModel, TerminalReplayModel } from '../../contracts/model.mjs';

const modes = [{ width: 3840, height: 2160 }, { width: 1920, height: 1080 }];
const desktop = () => new DesktopModel({ sessionId: 'session', epoch: 'epoch', leaseId: 'lease', permissions: ['desktop.view', 'desktop.control', 'desktop.resize'] });
const frame = (d, frameId = 1, keyframe = true) => ({ epoch: d.epoch, topology: d.topology, generation: d.generation, frameId, keyframe });
const resize = (d, requestId, mode) => ({ requestId, leaseId: 'lease', topology: d.topology, ...mode });

test('dedicated attachment streams enforce binding, permissions and closure', () => {
  const stream = new AttachmentStreamModel();
  const session = { sessionId: 's', sessionEpoch: 'e' };
  const binding = { ...session, attachmentId: 'a', permissions: ['terminal.open'], expiresAt: 45_000 };
  assert.throws(() => stream.authorize(session, 'terminal.open', 0), /unauthorized/);
  stream.bind(binding);
  assert.throws(() => stream.bind(binding), /forbidden/);
  stream.authorize(session, 'terminal.open', 1);
  assert.throws(() => stream.authorize({ ...session, sessionId: 'other' }, 'terminal.open', 1), /forbidden/);
  assert.throws(() => stream.authorize({ ...session, sessionEpoch: 'old' }, 'terminal.open', 1), /stale_epoch/);
  assert.throws(() => stream.authorize(session, 'terminal.input', 1), /forbidden/);
  assert.throws(() => stream.authorize(session, 'terminal.open', 45_000), /expired/);
  assert.throws(() => stream.bind(binding), /forbidden/);
  const fresh = new AttachmentStreamModel();
  fresh.bind(binding); fresh.revoke();
  assert.throws(() => fresh.authorize(session, 'terminal.open', 2), /unauthorized/);
});

test('grant binds identity/session, expires, and is consumed only once', () => {
  const g = new GrantModel();
  g.prepare('hash', 'issuer/subject/session/epoch/permissions', 0, 30000);
  assert.throws(() => g.redeem('hash', 'other', 1), /forbidden/);
  g.redeem('hash', 'issuer/subject/session/epoch/permissions', 29999);
  assert.throws(() => g.redeem('hash', 'issuer/subject/session/epoch/permissions', 29999), /replayed_grant/);
  g.prepare('second', 'binding', 0, 30000);
  assert.throws(() => g.redeem('second', 'binding', 30000), /expired/);
});

test('4K ->1080p ->4K retains desktop identity and terminal bytes', () => {
  const d = desktop(), t = new TerminalReplayModel();
  const identity = [d.sessionId, d.epoch];
  d.streamReady(1); d.frame(frame(d));
  d.input({ leaseId: 'lease', topology: 1, sequence: 1, key: 'shift' });
  t.output(Buffer.from('before'), 0);
  for (const [index, mode] of [modes[1], modes[0]].entries()) {
    const old = frame(d, d.lastFrame + 1);
    d.beginResize(resize(d, 'resize-' + index, mode), modes);
    assert.equal(d.held.size, 0);
    assert.throws(() => d.input({ leaseId: 'lease', topology: d.topology, sequence: 99, x: 1, y: 1 }), /stale_topology/);
    d.finishResize(true, mode.width, mode.height);
    assert.throws(() => d.frame(old), /stale_topology/);
    assert.throws(() => d.frame(frame(d)), /decoder_failed/);
    d.streamReady(d.generation);
    assert.throws(() => d.frame(frame(d, 1, false)), /decoder_failed/);
    d.frame(frame(d));
    d.input({ leaseId: 'lease', topology: d.topology, sequence: index + 2, x: mode.width - 1, y: mode.height - 1 });
    t.output(Buffer.from('during'), index + 1);
  }
  assert.deepEqual([d.sessionId, d.epoch], identity);
  assert.equal(d.width, 3840); assert.equal(d.height, 2160);
  assert.equal(Buffer.concat(t.resume(0, 3).chunks.map(c => c.data)).toString(), 'beforeduringduring');
});

test('resize permission, stale topology, supported modes and idempotency', () => {
  const d = desktop();
  d.permissions.delete('desktop.resize');
  assert.throws(() => d.beginResize(resize(d, 'r', modes[1]), modes), /forbidden/);
  d.permissions.add('desktop.resize');
  assert.throws(() => d.beginResize({ ...resize(d, 'r', modes[1]), topology: 0 }, modes), /stale_topology/);
  assert.throws(() => d.beginResize(resize(d, 'r', { width: 1234, height: 987 }), modes), /invalid_argument/);
  const request = resize(d, 'r', modes[1]);
  d.beginResize(request, modes);
  assert.throws(() => d.beginResize(resize(d, 'other', modes[0]), modes), /control_conflict/);
  const result = d.finishResize(true, 1920, 1080);
  assert.deepEqual(d.beginResize(request, modes), result);
  assert.throws(() => d.beginResize({ ...request, width: 3840 }, modes), /invalid_argument/);
});

test('failed resize preserves geometry; failed rollback reports actual geometry', () => {
  const d = desktop();
  d.beginResize(resize(d, 'r1', modes[1]), modes);
  assert.equal(d.finishResize(false).status, 'rejected');
  assert.equal(d.topology, 1); assert.equal(d.width, 3840);
  d.beginResize(resize(d, 'r2', modes[1]), modes);
  const result = d.finishResize(false, 1280, 720);
  assert.equal(result.status, 'rejected'); assert.equal(d.topology, 2); assert.equal(result.width, 1280);
});

test('loss requires keyframe; old epochs/sequences and revoked input fail', () => {
  const d = desktop();
  d.streamReady(1); d.frame(frame(d)); d.loseReference();
  assert.throws(() => d.frame(frame(d, 2, false)), /decoder_failed/);
  d.frame(frame(d, 3));
  assert.throws(() => d.frame(frame(d, 2)), /stale_sequence/);
  assert.throws(() => d.frame({ ...frame(d, 4), epoch: 'old' }), /stale_epoch/);
  d.input({ leaseId: 'lease', topology: 1, sequence: 1, key: 'shift' });
  assert.throws(() => d.input({ leaseId: 'lease', topology: 1, sequence: 1 }), /stale_sequence/);
  d.revokeControl(); assert.equal(d.held.size, 0);
  assert.throws(() => d.input({ leaseId: 'lease', topology: 1, sequence: 2 }), /control_conflict/);
});

test('terminal replay is byte-preserving, bounded, gap-aware and ordered', () => {
  const t = new TerminalReplayModel(4, 120000);
  t.output(Buffer.from([0xff, 0]), 0); t.output(Buffer.from([0xc3, 0xa9]), 1);
  assert.deepEqual(Buffer.concat(t.resume(0, 2).chunks.map(c => c.data)), Buffer.from([0xff, 0, 0xc3, 0xa9]));
  t.output(Buffer.from([1, 2]), 2);
  assert.ok(t.bytes <= 4); assert.equal(t.resume(0, 3).gap, true);
  assert.deepEqual(t.resume(2, 3).chunks.map(c => c.sequence), [3]);
  assert.equal(t.resume(0, 120002).chunks.length, 0);
  t.input(1); assert.throws(() => t.input(1), /stale_sequence/);
  assert.throws(() => t.input(3), /stale_sequence/);
  assert.throws(() => t.resume(4, 120003), /invalid_argument/);
});
