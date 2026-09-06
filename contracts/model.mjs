// Executable specification for selected invariants. Not production authorization code.
export class AttachmentStreamModel {
  constructor() { this.binding = null; this.closed = false; }
  bind({ attachmentId, sessionId, sessionEpoch, permissions, expiresAt }) {
    if (this.closed || this.binding) throw Error('forbidden');
    this.binding = { attachmentId, sessionId, sessionEpoch, permissions: new Set(permissions), expiresAt };
  }
  authorize({ sessionId, sessionEpoch }, permission, now) {
    if (this.closed || !this.binding) throw Error('unauthorized');
    if (now >= this.binding.expiresAt) { this.closed = true; throw Error('expired'); }
    if (sessionId !== this.binding.sessionId) throw Error('forbidden');
    if (sessionEpoch !== this.binding.sessionEpoch) throw Error('stale_epoch');
    if (!this.binding.permissions.has(permission)) throw Error('forbidden');
  }
  revoke() { this.closed = true; }
}

export class GrantModel {
  constructor() { this.grants = new Map(); }
  prepare(hash, binding, now, ttl) {
    if (ttl <= 0 || ttl > 30_000 || this.grants.has(hash)) throw Error('invalid_argument');
    this.grants.set(hash, { binding, expires: now + ttl, used: false });
  }
  redeem(hash, binding, now) {
    const grant = this.grants.get(hash);
    if (!grant || grant.binding !== binding) throw Error('forbidden');
    if (grant.used) throw Error('replayed_grant');
    if (now >= grant.expires) throw Error('expired');
    grant.used = true;
  }
}

export class DesktopModel {
  constructor({ sessionId, epoch, leaseId, permissions, width = 3840, height = 2160 }) {
    Object.assign(this, { sessionId, epoch, leaseId, width, height });
    this.permissions = new Set(permissions);
    this.topology = 1;
    this.generation = 1;
    this.lastInput = 0;
    this.lastFrame = 0;
    this.ready = false;
    this.needsKeyframe = true;
    this.resize = null;
    this.resizeResults = new Map();
    this.held = new Set();
  }
  requireLease(leaseId, permission) {
    if (!this.leaseId || this.leaseId !== leaseId) throw Error('control_conflict');
    if (!this.permissions.has(permission)) throw Error('forbidden');
  }
  input({ leaseId, topology, sequence, x, y, key }) {
    this.requireLease(leaseId, 'desktop.control');
    if (this.resize || topology !== this.topology) throw Error('stale_topology');
    if (sequence <= this.lastInput) throw Error('stale_sequence');
    if (x !== undefined && (x < 0 || x >= this.width || y < 0 || y >= this.height)) throw Error('invalid_argument');
    this.lastInput = sequence;
    if (key) this.held.add(key);
  }
  beginResize({ requestId, leaseId, topology, width, height }, modes) {
    this.requireLease(leaseId, 'desktop.resize');
    const prior = this.resizeResults.get(requestId);
    if (prior) {
      if (prior.requestedWidth !== width || prior.requestedHeight !== height) throw Error('invalid_argument');
      return prior;
    }
    if (this.resize) throw Error('control_conflict');
    if (topology !== this.topology) throw Error('stale_topology');
    if (!modes.some(m => m.width === width && m.height === height)) throw Error('invalid_argument');
    this.held.clear();
    this.resize = { requestId, width, height };
  }
  finishResize(applied, actualWidth = this.width, actualHeight = this.height) {
    if (!this.resize) throw Error('invalid_argument');
    const requested = this.resize;
    if (applied && (actualWidth !== requested.width || actualHeight !== requested.height)) throw Error('invalid_argument');
    const changed = actualWidth !== this.width || actualHeight !== this.height;
    if (changed) {
      this.width = actualWidth; this.height = actualHeight;
      this.topology++; this.generation++;
      this.lastFrame = 0; this.ready = false; this.needsKeyframe = true;
    }
    const result = {
      status: applied ? 'applied' : 'rejected',
      width: this.width, height: this.height, topology: this.topology,
      requestedWidth: requested.width, requestedHeight: requested.height,
    };
    this.resizeResults.set(requested.requestId, result);
    this.resize = null;
    return result;
  }
  streamReady(generation) {
    if (generation !== this.generation) throw Error('stale_sequence');
    this.ready = true;
  }
  frame({ epoch, topology, generation, frameId, keyframe }) {
    if (epoch !== this.epoch) throw Error('stale_epoch');
    if (topology !== this.topology || this.resize) throw Error('stale_topology');
    if (generation !== this.generation || frameId <= this.lastFrame) throw Error('stale_sequence');
    if (!this.ready || (this.needsKeyframe && !keyframe)) throw Error('decoder_failed');
    this.needsKeyframe = false; this.lastFrame = frameId;
  }
  loseReference() { this.needsKeyframe = true; }
  revokeControl() { this.leaseId = null; this.held.clear(); }
}

export class TerminalReplayModel {
  constructor(maxBytes = 4 * 1024 * 1024, retentionMs = 120_000) {
    this.maxBytes = maxBytes; this.retentionMs = retentionMs;
    this.chunks = []; this.sequence = 0; this.bytes = 0; this.lastInput = 0;
  }
  prune(now) {
    while (this.chunks.length && (this.bytes > this.maxBytes || now - this.chunks[0].time >= this.retentionMs)) {
      this.bytes -= this.chunks.shift().data.length;
    }
  }
  output(data, now) {
    if (!Buffer.isBuffer(data) || !data.length || data.length > 65536) throw Error('invalid_argument');
    this.chunks.push({ sequence: ++this.sequence, time: now, data: Buffer.from(data) });
    this.bytes += data.length; this.prune(now);
  }
  input(sequence) {
    if (sequence !== this.lastInput + 1) throw Error('stale_sequence');
    this.lastInput = sequence;
  }
  resume(last, now) {
    if (last < 0 || last > this.sequence) throw Error('invalid_argument');
    this.prune(now);
    const earliest = this.chunks[0]?.sequence ?? this.sequence + 1;
    return { gap: last + 1 < earliest, chunks: this.chunks.filter(c => c.sequence > last) };
  }
}
