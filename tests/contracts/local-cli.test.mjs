import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import { uidPolicy, validateRecord, validateSnapshot, LocalLeaseModel, MAX_PNG } from '../../contracts/local-cli-model.mjs';

const read = path => JSON.parse(readFileSync(new URL('../../' + path, import.meta.url), 'utf8'));
const ajv = new Ajv({ strict: true, allErrors: true });
addFormats(ajv);
ajv.addSchema(read('contracts/schemas/common.schema.json'));
const schema = read('contracts/schemas/local-cli.schema.json');
const validate = ajv.compile(schema);
const examples = read('contracts/fixtures/local-cli.json');
for (const message of examples) test(`local CLI accepts ${message.type}`, () => assert.ok(validate(message), ajv.errorsText(validate.errors)));

test('local CLI has one example per variant, separate protocol and strict fields', () => {
  assert.deepEqual(examples.map(x => x.type).sort(), schema.oneOf.map(x => x.properties.type.const).sort());
  for (const message of examples) {
    assert.equal(validate({ ...message, version: '0.1.0' }), false);
    assert.equal(validate({ ...message, protocol: 'dwdesktop' }), false);
    assert.equal(validate({ ...message, principal: 'untrusted' }), false);
    if (message.requestMessageId) {
      const copy = structuredClone(message); delete copy.requestMessageId;
      assert.equal(validate(copy), false, message.type);
    }
  }
});

test('local schema rejects PNG bounds and invalid terminal exit states', () => {
  const sample = examples.find(x => x.type === 'screenshot.result');
  for (const payload of [{ width: 4097 }, { height: 0 }, { payloadBytes: MAX_PNG + 1 }, { pixelFormat: 'rgb8' }]) {
    assert.equal(validate({ ...sample, payload: { ...sample.payload, ...payload } }), false);
  }
  const exit = examples.find(x => x.type === 'terminal.exit');
  assert.equal(validate({ ...exit, payload: { ...exit.payload, exitCode: null, signal: null } }), false);
  assert.equal(validate({ ...exit, payload: { ...exit.payload, exitCode: 0, signal: 9 } }), false);
});

test('UID policy denies unmapped peers and is looked up anew', () => {
  const policies = new Map([[1000, { profile: 'pilot', permissions: ['desktop.view'] }]]);
  assert.throws(() => uidPolicy(policies, 0), /unauthorized/);
  assert.equal(uidPolicy(policies, 1000).profile, 'pilot');
  policies.delete(1000);
  assert.throws(() => uidPolicy(policies, 1000), /unauthorized/);
});

const snapshot = { snapshotId: 'snap', sessionEpoch: 'epoch', displayId: 'display', width: 3840, height: 2160, payloadBytes: 1024, captureTimeUs: 100, topologyRevision: 1 };
test('fresh native capture and framing limits are independent of compressed size', () => {
  validateSnapshot(snapshot, { width: 3840, height: 2160, pixelFormat: 'rgba8' }, 99, 1);
  assert.throws(() => validateSnapshot(snapshot, { width: 1920, height: 1080, pixelFormat: 'rgba8' }, 99, 1), /invalid_argument/);
  assert.throws(() => validateSnapshot(snapshot, { width: 3840, height: 2160, pixelFormat: 'rgba8' }, 101, 1), /capture_failed/);
  assert.throws(() => validateSnapshot(snapshot, { width: 3840, height: 2160, pixelFormat: 'rgba8' }, 99, 2), /stale_topology/);
  assert.throws(() => validateSnapshot({ ...snapshot, width: 100_000 }, {}, 99, 1), /invalid_argument/);
  validateRecord(65_536, 'screenshot.result', MAX_PNG);
  for (const args of [[65_537, 'screenshot.result', 1], [1, 'screenshot.result', MAX_PNG + 1], [1, 'screenshot.result', 0], [1, 'input.text', 1]]) assert.throws(() => validateRecord(...args), /invalid_argument/);
});

test('input lease, screenshot geometry, corners and replay fail closed', () => {
  const lease = new LocalLeaseModel(), permissions = new Set(['desktop.control']);
  assert.throws(() => lease.acquire('owner', new Set(['desktop.view']), 0), /forbidden/);
  assert.throws(() => lease.acquire('owner', permissions, 0, 180_000), /invalid_argument/);
  lease.acquire('owner', permissions, 0);
  assert.throws(() => lease.acquire('other', permissions, 1), /control_conflict/);
  let seq = 0;
  const req = overrides => ({ ...snapshot, inputSequence: seq + 1, x: 0, y: 0, ...overrides });
  for (const [x, y] of [[0, 0], [3839, 0], [0, 2159], [3839, 2159]]) {
    lease.input('owner', snapshot, req({ x, y }), 1, 1); seq++;
  }
  assert.throws(() => lease.input('owner', snapshot, req({ x: 3840 }), 1, 1), /invalid_argument/);
  assert.throws(() => lease.input('owner', snapshot, req({ inputSequence: seq }), 1, 1), /stale_sequence/);
  assert.throws(() => lease.input('owner', snapshot, req({}), 2, 1), /stale_topology/);
  assert.throws(() => lease.input('owner', snapshot, req({ sessionEpoch: 'old' }), 1, 1), /stale_epoch/);
  lease.input('owner', snapshot, req({ key: 'shift' }), 1, 1);
  assert.equal(lease.held.size, 1);
  assert.throws(() => lease.renew('owner', permissions, 45_000), /expired/);
  assert.equal(lease.held.size, 0);
  lease.acquire('owner', permissions, 45_001); lease.release();
  assert.throws(() => lease.check('owner', 45_002), /expired/);
});
