// Executable specification only; not production peer auth, PNG decoding or input injection.
export const MAX_HEADER = 65_536;
export const MAX_PNG = 67_108_864;
export function uidPolicy(policies, peerUid) {
  const policy = policies.get(peerUid);
  if (!policy || !policy.profile) throw Error('unauthorized');
  return { profile: policy.profile, permissions: new Set(policy.permissions) };
}
export function validateRecord(headerBytes, type, payloadBytes = 0) {
  if (!Number.isInteger(headerBytes) || headerBytes < 1 || headerBytes > MAX_HEADER) throw Error('invalid_argument');
  if (!Number.isInteger(payloadBytes) || payloadBytes < 0 || payloadBytes > MAX_PNG) throw Error('invalid_argument');
  if ((type === 'screenshot.result') !== (payloadBytes > 0)) throw Error('invalid_argument');
}
export function validateSnapshot(snapshot, decoded, acceptedAtUs, currentTopology) {
  const { width, height, payloadBytes, captureTimeUs, topologyRevision } = snapshot;
  if (![width, height].every(n => Number.isInteger(n) && n > 0 && n <= 4096)) throw Error('invalid_argument');
  if (BigInt(width) * BigInt(height) * 4n > BigInt(MAX_PNG)) throw Error('resource_exhausted');
  validateRecord(1, 'screenshot.result', payloadBytes);
  if (decoded.width !== width || decoded.height !== height || decoded.pixelFormat !== 'rgba8') throw Error('invalid_argument');
  if (captureTimeUs < acceptedAtUs) throw Error('capture_failed');
  if (topologyRevision !== currentTopology) throw Error('stale_topology');
}
export class LocalLeaseModel {
  constructor() { this.lease = null; this.held = new Set(); this.sequence = 0; }
  acquire(owner, permissions, now, ttl = 45_000) {
    if (!permissions.has('desktop.control')) throw Error('forbidden');
    if (ttl < 1 || ttl > 45_000) throw Error('invalid_argument');
    if (this.lease && now < this.lease.expires) throw Error('control_conflict');
    this.release(); this.lease = { owner, expires: now + ttl }; this.sequence = 0;
  }
  check(owner, now) {
    if (this.lease && now >= this.lease.expires) this.release();
    if (!this.lease || this.lease.owner !== owner) throw Error('expired');
  }
  renew(owner, permissions, now, ttl = 45_000) {
    this.check(owner, now);
    if (!permissions.has('desktop.control')) { this.release(); throw Error('forbidden'); }
    if (ttl < 1 || ttl > 45_000) throw Error('invalid_argument');
    this.lease.expires = now + ttl;
  }
  input(owner, snapshot, request, topology, now) {
    this.check(owner, now);
    if (snapshot.sessionEpoch !== request.sessionEpoch) throw Error('stale_epoch');
    if (snapshot.snapshotId !== request.snapshotId || snapshot.displayId !== request.displayId) throw Error('not_found');
    if (snapshot.topologyRevision !== topology || request.topologyRevision !== topology) throw Error('stale_topology');
    if (request.x !== undefined && (!Number.isInteger(request.x) || !Number.isInteger(request.y) || request.x < 0 || request.y < 0 || request.x >= snapshot.width || request.y >= snapshot.height)) throw Error('invalid_argument');
    if (request.inputSequence !== this.sequence + 1) throw Error('stale_sequence');
    this.sequence = request.inputSequence;
    if (request.key) this.held.add(request.key);
  }
  release() { this.held.clear(); this.lease = null; }
}
