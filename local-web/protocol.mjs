export class Failure extends Error {
  constructor(code, status = 503) { super(code); this.code = code; this.status = status; }
}
export const MAX_HEADER = 65536, MAX_PNG = 67108864;
const fail = () => { throw new Failure('invalid_argument', 400); };
export function object(value, fields) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length !== fields.length || fields.some(key => !Object.hasOwn(value, key))) fail();
}
export function integer(value, min = 0, max = Number.MAX_SAFE_INTEGER) { if (!Number.isSafeInteger(value) || value < min || value > max) fail(); }
export function text(value, max = 128) { if (typeof value !== 'string' || !value.length || value.length > max) fail(); }
export function uuid(value) { if (typeof value !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)) fail(); }
export function choice(value, choices) { if (!choices.includes(value)) fail(); }
function boolean(value) { if (typeof value !== 'boolean') fail(); }

// Strict JSON, including duplicate keys that JSON.parse would otherwise discard.
export function parseJson(bytes) {
  let source;
  try { source = new TextDecoder('utf-8', { fatal: true }).decode(bytes); } catch { fail(); }
  let at = 0;
  const white = () => { while (/[\x20\t\r\n]/.test(source[at] ?? '') && at < source.length) at++; };
  function string() {
    const start = at++;
    while (at < source.length) {
      const ch = source[at++];
      if (ch === '\\') { at++; continue; }
      if (ch === '"') { try { return JSON.parse(source.slice(start, at)); } catch { fail(); } }
    }
    fail();
  }
  function value(depth) {
    if (depth > 64) fail(); white();
    if (source[at] === '"') return string();
    if (source[at] === '{') {
      at++; white(); const result = {}, keys = new Set();
      if (source[at] === '}') { at++; return result; }
      for (;;) {
        white(); if (source[at] !== '"') fail();
        const key = string(); if (keys.has(key)) fail(); keys.add(key);
        white(); if (source[at++] !== ':') fail();
        Object.defineProperty(result, key, { value: value(depth + 1), enumerable: true, writable: true, configurable: true });
        white(); const end = source[at++]; if (end === '}') return result; if (end !== ',') fail();
      }
    }
    if (source[at] === '[') {
      at++; white(); const result = [];
      if (source[at] === ']') { at++; return result; }
      for (;;) { result.push(value(depth + 1)); white(); const end = source[at++]; if (end === ']') return result; if (end !== ',') fail(); }
    }
    const match = /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(source.slice(at));
    if (!match) fail(); at += match[0].length;
    const result = JSON.parse(match[0]); if (typeof result === 'number' && !Number.isFinite(result)) fail(); return result;
  }
  const result = value(0); white(); if (at !== source.length) fail(); return result;
}

const permissions = ['desktop.view', 'desktop.control', 'desktop.resize', 'clipboard.read', 'clipboard.write', 'terminal.open', 'terminal.input'];
const errors = ['unsupported_version','invalid_argument','unauthorized','forbidden','expired','replayed_grant','control_conflict','stale_epoch','stale_topology','stale_sequence','not_found','resource_exhausted','history_unavailable','worker_unavailable','decoder_failed','unsupported_capability','capture_failed','initialization_failed'];
function resolution(mode) { object(mode, ['width', 'height']); integer(mode.width, 1, 4096); integer(mode.height, 1, 4096); }
function display(d) {
  object(d, ['displayId','width','height','topologyRevision','cursorEmbedded','canResize','availableResolutions']);
  text(d.displayId); integer(d.width,1,4096); integer(d.height,1,4096); integer(d.topologyRevision,1); boolean(d.cursorEmbedded); boolean(d.canResize);
  if (!Array.isArray(d.availableResolutions) || d.availableResolutions.length > 128) fail(); d.availableResolutions.forEach(resolution);
}
export function validateReply(message, request, expected) {
  try {
    const unbound = ['error', 'describe.result'].includes(message?.type);
    object(message, ['protocol','version','type','messageId','requestMessageId','payload', ...(unbound ? [] : ['sessionId','sessionEpoch'])]);
    choice(message.protocol,['dwdesktop.local']); choice(message.version,['0.2.0']); uuid(message.messageId); uuid(message.requestMessageId);
    if (message.requestMessageId !== request.messageId || ![expected,'error'].includes(message.type)) fail();
    if (!unbound) {
      uuid(message.sessionId); uuid(message.sessionEpoch);
      if (request.sessionId && (message.sessionId !== request.sessionId || message.sessionEpoch !== request.sessionEpoch)) fail();
    }
    const p = message.payload;
    switch (message.type) {
      case 'error': object(p,['code','message','retryable']); choice(p.code,errors); if (typeof p.message !== 'string' || p.message.length > 512) fail(); boolean(p.retryable); break;
      case 'describe.result':
        object(p,['workerId','peerUid','osAccountProfile','permissions','displays']); uuid(p.workerId); integer(p.peerUid,0,4294967295); text(p.osAccountProfile);
        if (!Array.isArray(p.permissions) || p.permissions.length > 7 || new Set(p.permissions).size !== p.permissions.length) fail(); p.permissions.forEach(x => choice(x,permissions));
        if (!Array.isArray(p.displays) || p.displays.length > 16) fail(); p.displays.forEach(display); break;
      case 'session.opened': object(p,['state']); choice(p.state,['ready']); break;
      case 'ack': object(p,[]); break;
      case 'control.granted': object(p,['leaseId','validForMs']); uuid(p.leaseId); integer(p.validForMs,1,45000); break;
      case 'screenshot.result':
        object(p,['snapshotId','displayId','topologyRevision','width','height','format','pixelFormat','cursorEmbedded','captureTimeUs','payloadBytes']);
        uuid(p.snapshotId); text(p.displayId); integer(p.topologyRevision,1); integer(p.width,1,4096); integer(p.height,1,4096); integer(p.captureTimeUs); integer(p.payloadBytes,1,MAX_PNG);
        choice(p.format,['png']); choice(p.pixelFormat,['rgba8']); boolean(p.cursorEmbedded);
        if (p.width * p.height * 4 > MAX_PNG || p.displayId !== request.payload.displayId || p.cursorEmbedded !== request.payload.includeCursor) fail(); break;
      case 'display.resize.result':
        object(p,['displayId','status','width','height','topologyRevision','reason']); text(p.displayId); choice(p.status,['applied','rejected']); integer(p.width,1,4096); integer(p.height,1,4096); integer(p.topologyRevision,1); choice(p.reason,['none','unsupported_mode','os_failed','rollback_failed']);
        if (p.displayId !== request.payload.displayId || (p.status === 'applied') !== (p.reason === 'none')) fail();
        if (p.status === 'applied' && (p.width !== request.payload.width || p.height !== request.payload.height)) fail(); break;
      default: fail();
    }
    return message;
  } catch { throw new Failure('worker_protocol_error'); }
}

export function daemonFailure(code) {
  const status = { forbidden:403, unauthorized:403, invalid_argument:400, not_found:404, stale_epoch:409, stale_topology:409, stale_sequence:409, control_conflict:409, expired:409, resource_exhausted:429, unsupported_capability:422 }[code] ?? 503;
  return new Failure(code,status);
}
