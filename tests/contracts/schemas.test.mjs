import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';

const read = path => JSON.parse(readFileSync(new URL('../../' + path, import.meta.url), 'utf8'));
const ajv = new Ajv({ strict: true, allErrors: true });
addFormats(ajv);
const schemas = Object.fromEntries(['common', 'control', 'media', 'terminal'].map(name => [name, read(`contracts/schemas/${name}.schema.json`)]));
for (const schema of Object.values(schemas)) ajv.addSchema(schema);
const validators = Object.fromEntries(['control', 'media', 'terminal'].map(name => [name, ajv.getSchema(schemas[name].$id)]));
const examples = read('contracts/fixtures/examples.json');

for (const { schema, message } of examples) {
  test(`accept ${message.type}`, () => {
    const validate = validators[schema];
    assert.ok(validate(message), ajv.errorsText(validate.errors));
  });
}
for (const { schema, message, why } of read('contracts/fixtures/negative.json')) {
  test(`reject ${message.type}: ${why}`, () => assert.equal(validators[schema](message), false));
}
test('every message kind has exactly one positive fixture', () => {
  for (const [name, schema] of Object.entries(schemas)) {
    if (!schema.oneOf) continue;
    const expected = schema.oneOf.map(v => v.properties.type.const).sort();
    const actual = examples.filter(v => v.schema === name).map(v => v.message.type).sort();
    assert.deepEqual(actual, expected);
  }
});

test('typed control and terminal replies require request correlation', () => {
  const replies = ['worker.capabilities', 'session.created', 'attachment.authorized', 'control.granted', 'terminal.opened', 'terminal.inputAck'];
  for (const type of replies) {
    const example = examples.find(v => v.message.type === type);
    const message = structuredClone(example.message);
    assert.ok(message.payload.requestMessageId);
    delete message.payload.requestMessageId;
    assert.equal(validators[example.schema](message), false, type);
  }
});

test('session creation cannot report pending initialization as success', () => {
  const example = examples.find(v => v.message.type === 'session.created');
  for (const state of ['starting', 'failed', 'closed']) {
    const message = structuredClone(example.message);
    message.payload.state = state;
    assert.equal(validators.control(message), false, state);
  }
});
