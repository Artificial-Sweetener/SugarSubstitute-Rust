// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Execute production JavaScript; fake only browser/native boundaries.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { Script, createContext } from 'node:vm';
const sourcePath = process.argv[2] ?? new URL('../src/main_actor/panel/bridge.js', import.meta.url);
const source = readFileSync(sourcePath, 'utf8');
const policySource = readFileSync(new URL('../src/main_actor/panel/bridge_protocol.js', import.meta.url), 'utf8');
const bulkSource = readFileSync(new URL('../src/main_actor/panel/bridge_bulk.js', import.meta.url), 'utf8');
// Match the host's composition: the bulk expression receives the absolute reserved path.
const bulkFor = path => `${bulkSource.trimEnd()}(${JSON.stringify(path)});\n`;
const pageOrigin = 'http://127.0.0.1:8188';
const bulkPath = '/__sugarsubstitute/bulk/';
const sources = [policySource, bulkFor(bulkPath), source];
const binding = '__sugarcubes_substitute_send_v3';
const host = 'ai.artificialsweetener.sugarsubstitute';
const session = 'session-12345678';
const maxTicket = Number.MAX_SAFE_INTEGER;
const address = { sessionId: 'canvas:w', viewId: 'view:a', windowId: 'window:a', attachmentId: 'attachment:a', generation: 1 };
const canvas = (requestId = 1, type = 'context.request', extra = {}) =>
  ({ protocol: 'sugarcubes.canvas', version: 3, session, requestId, type, ...extra });
const control = (type = 'ready', extra = {}) => ({ protocol: 'sugarcubes.panel', version: 3, session, type, ...extra });
const selection = (text = '') => canvas(1, 'surface.select', { address, revision: 1, itemId: 'result:one', file: { filename: text, subfolder: '', type: 'output' } });
const viewport = (requestId = 1, command = { type: 'read' }) =>
  canvas(requestId, 'surface.viewport.request', { address, revision: 1, command });
const viewportReply = (requestId = 1) => canvas(requestId, 'surface.viewport.reply', {
  address, revision: 1, status: 'applied', message: '',
  viewport: { mode: 'fit', physicalScale: .5, dimensions: { width: 640, height: 480 } },
});
const normalize = value => JSON.parse(JSON.stringify(value));
function transportError(error, code) {
  assert.equal(error.message, '');
  assert.equal(error.cause.code, `panel.transport.${code}`);
  return true;
}
function fixture(post, scripts = sources, fetch = undefined, origin = pageOrigin) {
  const sent = [], received = [], events = new Map(), window = {};
  window.top = window;
  const context = createContext({ window, URLSearchParams, TextEncoder, TextDecoder, AbortController, fetch,
    location: { origin, search: `?sc-panel-host=${host}&sc-panel-session=${session}&sc-panel-transport=native` },
    [binding]: payload => { if (post) post(payload); sent.push(payload); },
    addEventListener: (name, listener) => events.set(name, listener) });
  for (const script of scripts) new Script(script).runInContext(context);
  const bridge = context.sugarcubesPanelHost;
  bridge?.subscribe(value => received.push(normalize(value)));
  return { context, bridge, sent, received, events, receipt: context.__sugarcubes_substitute_transport_v1, deliver: context.__sugarcubes_substitute_deliver_v3 };
}
function envelope(payload) {
  return JSON.stringify({ name: binding, payload, executionContextId: 0 }).replace('"executionContextId":0', '"executionContextId":-9223372036854775808');
}
const framed = (message, lane = 'o', ticket = maxTicket) => `1:${lane}:${ticket}:${JSON.stringify(message)}`;
const grant = 'ab'.repeat(16);
const bulkBase = `${pageOrigin}${bulkPath}`;
const bulkEnvelope = (token, bytes) => JSON.stringify({ transport: 'sugarsubstitute.bulk', version: 1, token, bytes });
const settle = () => new Promise(resolve => setImmediate(resolve));
// Fake only the browser fetch boundary; `respond` returns a response or throws for a network failure.
function bulkFetch(respond = () => ({ status: 204 })) {
  const requests = [];
  const fetch = (url, init) => {
    requests.push({ url, init });
    return Promise.resolve().then(() => respond(url, init));
  };
  return { fetch, requests };
}
function body(text, status = 200) {
  const bytes = Buffer.from(text);
  return { status, arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) };
}
function admit(message, lane = 'o') {
  const f = fixture();
  assert.equal(f.receipt('g', lane, maxTicket), true);
  f.bridge.send(message);
  return f;
}

test('each bootstrap script fits the unchanged native registration command', () => {
  for (const source of sources)
    assert.ok(Buffer.byteLength(JSON.stringify({ source })) <= 8192);
});
test('the bridge carries only its identity and channel; host facts arrive in the description', () => {
  assert.deepEqual(Object.keys(fixture().bridge).sort(), ['host', 'send', 'session', 'subscribe']);
});
test('the host description is a version-one notification for the current session only', () => {
  const f = fixture();
  const description = { protocol: 'sugarcubes.host', version: 1, session, type: 'description',
    protocols: [{ name: 'sugarcubes.panel', version: 3 }], capabilities: [] };
  for (const value of [{ ...description, version: 3 }, { ...description, session: 'another-session' },
    { ...description, type: 'configure' }])
    assert.equal(f.deliver(value), false);
  assert.deepEqual(f.received, []);
  assert.equal(f.deliver(description), true);
  assert.equal(f.deliver(control('configure')), true);
  assert.deepEqual(f.received.map(value => value.type), ['description', 'configure']);
});
test('one ordered evaluation hands over the configure only after the page accepted the description', () => {
  // Match the host's ordered delivery: `deliver(description)&&deliver(configure)`.
  const ordered = (first, second) => new Script(`globalThis.__sugarcubes_substitute_deliver_v3(${
    JSON.stringify(first)})&&globalThis.__sugarcubes_substitute_deliver_v3(${JSON.stringify(second)})`);
  const description = { protocol: 'sugarcubes.host', version: 1, session, type: 'description',
    protocols: [{ name: 'sugarcubes.panel', version: 3 }], capabilities: [] };
  const refused = fixture();
  assert.equal(ordered({ ...description, session: 'another-session' }, control('configure'))
    .runInContext(refused.context), false);
  assert.deepEqual(refused.received, []);
  const accepted = fixture();
  assert.equal(ordered(description, control('configure')).runInContext(accepted.context), true);
  assert.deepEqual(accepted.received.map(value => value.type), ['description', 'configure']);
});
test('escaped selection beyond the native frame uploads before custody and later small requests stay inline', async () => {
  const transfer = bulkFetch(), f = fixture(undefined, sources, transfer.fetch), request = selection('\\'.repeat(3800));
  request.file.subfolder = '\\'.repeat(3800);
  assert.ok(Buffer.byteLength(JSON.stringify(request)) <= 16384);
  assert.ok(envelope(framed(request)).length + 1 > 16384);
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(request);
  assert.deepEqual(f.sent, []);
  await settle();
  assert.equal(transfer.requests.length, 1);
  assert.equal(transfer.requests[0].init.body, JSON.stringify(request));
  assert.deepEqual(f.sent, [`1:o:1:${bulkEnvelope(grant, Buffer.byteLength(JSON.stringify(request)))}`]);
  f.receipt('g', 'c', 1);
  f.bridge.send(control());
  assert.deepEqual(f.sent.slice(1), [framed(control(), 'c', 1)]);
});
test('UTF-16 boundary includes private prefix, terminator and maximum context identity', () => {
  const overhead = envelope(framed(selection())).length + 1;
  const exact = selection('x'.repeat(16384 - overhead)), f = admit(exact);
  assert.equal(envelope(f.sent[0]).length + 1, 16384);
  assert.equal(admit(selection(`${exact.file.filename}x`)).sent.length, 0);
});
// Well-formed JSON has UTF16 units <= UTF8 bytes <= 3 * UTF16 units.
// ASCII framing/escaping preserves bytes minus units, so these two 16384
// bounds imply native UTF8 <= 27305, within its independent 32768-byte limit.
for (const character of ['\\', '"', '\0', '\n', '漢', '🙂', '\u2028', '\u2029', '漢\\🙂"\n', '\ud800漢\u07ff']) {
  test(`admission preserves ${JSON.stringify(character)} inline up to the first native limit`, async () => {
    let first = 0, last = 20000;
    while (first < last) {
      const middle = Math.ceil((first + last) / 2);
      if (admit(selection(character.repeat(middle))).sent.length === 1) first = middle;
      else last = middle - 1;
    }
    const exact = selection(character.repeat(first)), f = admit(exact);
    const payload = JSON.stringify(exact), native = envelope(f.sent[0]);
    assert.equal(f.sent[0], framed(exact));
    assert.ok(Buffer.byteLength(payload) <= 16384);
    assert.ok(payload.length <= 16384);
    assert.ok(native.length + 1 <= 16384);
    assert.equal(Buffer.byteLength(native) - native.length, Buffer.byteLength(payload) - payload.length);
    assert.ok(Buffer.byteLength(native) <= 27305);
    assert.ok(Buffer.byteLength(native) <= 32768);
    const transfer = bulkFetch(), next = fixture(undefined, sources, transfer.fetch);
    const beyond = selection(character.repeat(first + 1));
    next.receipt('g', 'o', 1, grant);
    next.bridge.send(beyond);
    assert.deepEqual(next.sent, []);
    await settle();
    assert.equal(transfer.requests[0].init.body, JSON.stringify(beyond));
    assert.deepEqual(next.sent, [`1:o:1:${bulkEnvelope(grant, Buffer.byteLength(JSON.stringify(beyond)))}`]);
  });
}test('control grant is independent and configure can start context before applied', () => {
  const f = fixture();
  f.bridge.send(control());
  assert.equal(f.sent.length, 0);
  f.bridge.subscribe(value => {
    if (value.type === 'configure') { f.bridge.send(canvas()); f.bridge.send(control('applied')); }
  });
  f.deliver(control('configure'));
  f.receipt('g', 'c', 1);
  assert.equal(f.sent[0], framed(control(), 'c', 1));
  f.receipt('a', 'c', 1);
  f.receipt('g', 'c', 2);
  assert.equal(f.sent[1], framed(control('applied'), 'c', 2));
  f.receipt('g', 'o', 1);
  assert.equal(f.sent[2], framed(canvas(), 'o', 1));
});
test('one active plus eight waiting ordinary rejects duplicate identities and overload', () => {
  const f = admit(canvas());
  for (let id = 2; id <= 9; id++) f.bridge.send(canvas(id));
  for (const id of [1, 5, 9]) assert.throws(() => f.bridge.send(canvas(id)), error => transportError(error, 'duplicate'));
  assert.throws(() => f.bridge.send(canvas(10)), error => transportError(error, 'capacity'));
  assert.equal(f.sent.length, 1);
  assert.equal(f.receipt('g', 'o', maxTicket - 1), false);
  assert.equal(f.receipt('a', 'o', maxTicket), false);
});
test('concurrent owners queue in order and a cancelled middle request leaves the rest intact', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas(1, 'surface.capabilities.request', { schema: 1 }));
  f.bridge.send(canvas(2, 'surface.presentation.request', { schema: 1, enabled: true }));
  f.bridge.send({ ...selection(), requestId: 3 });
  f.bridge.send(canvas(4, 'surface.presentation.request', { schema: 1, enabled: true }));
  f.bridge.send(canvas(2, 'cancel'));
  assert.deepEqual(f.received.map(value => [value.requestId, value.type, value.failure?.status]),
    [[2, 'error.reply', 'cancelled']]);
  f.deliver(canvas(1, 'surface.capabilities.reply', { status: 'available', capabilities: {}, message: '' }), 1);
  f.receipt('g', 'o', 2);
  f.deliver(canvas(3, 'surface.reply', { address, revision: 1, status: 'applied', message: '' }), 2);
  f.receipt('g', 'o', 3);
  assert.deepEqual(f.sent.map(payload => JSON.parse(payload.slice(payload.indexOf('{'))).type),
    ['surface.capabilities.request', 'surface.select', 'surface.presentation.request']);
});
test('a failed native binding answers every queued ordinary request', () => {
  const f = fixture(() => { throw new Error('binding failed'); });
  for (const id of [1, 2, 3]) f.bridge.send(canvas(id));
  assert.equal(f.receipt('g', 'o', 1), false);
  assert.deepEqual(f.received.map(value => [value.requestId, value.failure.status]),
    [[1, 'unavailable'], [2, 'unavailable'], [3, 'unavailable']]);
  assert.throws(() => f.bridge.send(canvas(4)), error => transportError(error, 'closed'));
});
test('terminal delivery retains custody until native grant including reentrant sends', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.bridge.subscribe(() => f.bridge.send(canvas(2)));
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), true);
  assert.equal(f.sent.length, 1);
  assert.equal(f.receipt('g', 'o', 2), true);
  assert.equal(f.sent[1], framed(canvas(2), 'o', 2));
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), false);
  assert.equal(f.received.length, 1);
});
test('waiting request owns a snapshot independent of caller mutation', () => {
  const f = fixture(), value = selection('original');
  f.bridge.send(value);
  value.file.filename = 'changed';
  f.receipt('g', 'o', 1);
  assert.equal(f.sent[0], framed(selection('original'), 'o', 1));
});
test('viewport requests retain framing and accept their exact successful state reply', () => {
  const request = viewport(), reply = viewportReply(), f = admit(request);
  assert.equal(f.sent[0], framed(request));
  assert.equal(f.deliver(reply, maxTicket), true);
  assert.deepEqual(f.received, [reply]);
  assert.equal(f.deliver(reply, maxTicket), false);
});

test('viewport success cannot use a state-free legacy reply or another named request reply', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(viewport());
  for (const type of ['context.reply', 'surface.capabilities.reply'])
    assert.equal(f.deliver(canvas(1, type), 1), false);
  for (const status of ['ready', 'applied', 'unsupported'])
    assert.equal(f.deliver(canvas(1, 'surface.reply', { address, revision: 1, status, message: '' }), 1), false);
  assert.equal(f.deliver(viewportReply(2), 1), false);
  assert.equal(f.deliver(viewportReply(), 2), false);
  assert.equal(f.receipt('g', 'o', 2), false);
  assert.deepEqual(f.received, []);
  assert.equal(f.deliver(viewportReply(), 1), true);
});

for (const status of ['stale', 'cancelled', 'unavailable', 'error']) {
  test(`viewport ${status} uses the existing correlated terminal surface reply`, () => {
    const f = admit(viewport());
    const reply = canvas(1, 'surface.reply', { address, revision: 1, status, message: '' });
    assert.equal(f.deliver(reply, maxTicket), true);
    assert.deepEqual(f.received, [reply]);
    assert.equal(f.deliver(viewportReply(), maxTicket), false);
  });
}

test('viewport cancellation waits for the real terminal reply after native forwarding', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.receipt('g', 'x', 2);
  f.bridge.send(viewport(1, { type: 'pan', x: .2, y: -.1, units: 'viewport' }));
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.sent.length, 2);
  assert.equal(f.receipt('a', 'x', 2), true);
  assert.deepEqual(f.received, []);
  assert.equal(f.receipt('g', 'o', 3), false);
  const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'cancelled', message: '' });
  assert.equal(f.deliver(reply, 1), true);
  assert.deepEqual(f.received, [reply]);
});

test('viewport admission refusal settles once and the next ordinary request can progress', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(viewport());
  f.bridge.send(canvas(2));
  assert.equal(f.receipt('r', 'o', 1), true);
  assert.equal(f.received[0].type, 'error.reply');
  assert.equal(f.received[0].failure.status, 'unavailable');
  assert.equal(f.deliver(viewportReply(), 1), false);
  assert.equal(f.receipt('g', 'o', 2), true);
  assert.equal(f.sent[1], framed(canvas(2), 'o', 2));
});

test('named successful reply routing does not broaden the ordinary request allowlist', () => {
  const f = fixture();
  for (const type of ['surface.unknown.request', 'surface.viewport.other.request', 'surface.viewport.reply'])
    assert.throws(() => f.bridge.send(canvas(1, type)), error => transportError(error, 'unsupported'));
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas(1, 'surface.capabilities.request'));
  assert.equal(f.deliver(viewportReply(), 1), false);
  assert.equal(f.deliver(canvas(1, 'surface.reply', { status: 'unavailable' }), 1), false);
  assert.equal(f.deliver(canvas(1, 'surface.capabilities.reply'), 1), true);
});

for (const type of ['context.request', 'surface.capabilities.request', 'surface.select', 'surface.viewport.request']) {
  test(`never-forwarded ${type} cancellation withdraws locally`, () => {
    const f = fixture(), surface = type === 'surface.select' || type === 'surface.viewport.request';
    const request = type === 'surface.select' ? selection() : type === 'surface.viewport.request' ? viewport() : canvas(1, type);
    f.bridge.send(request);
    f.bridge.send(canvas(1, 'cancel'));
    assert.equal(f.sent.length, 0);
    assert.equal(f.received.length, 1);
    const reply = f.received[0];
    assert.equal(reply.type, surface ? 'surface.reply' : 'error.reply');
    assert.equal(reply.status ?? reply.failure.status, 'cancelled');
    if (surface) { assert.deepEqual(reply.address, address); assert.equal(reply.revision, 1); }
    f.bridge.send(canvas(2));
    f.receipt('g', 'o', 1);
    assert.equal(f.sent[0], framed(canvas(2), 'o', 1));
    f.receipt('g', 'x', 1);
    assert.equal(f.sent.length, 1);
  });
}
test('forwarded cancellation waits for its lane and the actual terminal response', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(selection());
  f.bridge.send(canvas(1, 'cancel'));
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.received.length, 0);
  f.receipt('g', 'x', 1);
  assert.equal(f.sent[1], framed(canvas(1, 'cancel'), 'x', 1));
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.sent.length, 2);
  assert.equal(f.receipt('r', 'x', 1), true);
  assert.equal(f.received.length, 0);
  f.receipt('g', 'x', 2);
  assert.equal(f.sent[2], framed(canvas(1, 'cancel'), 'x', 2));
  const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'cancelled', message: '' });
  assert.equal(f.deliver(reply, 1), true);
  assert.deepEqual(f.received, [reply]);
  assert.equal(f.receipt('a', 'x', 2), true);
  f.receipt('g', 'x', 3);
  assert.equal(f.sent.length, 3);
});
test('terminal discards a waiting cancellation without fabricating another send', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.bridge.send(canvas(1, 'cancel'));
  f.deliver(canvas(1, 'context.reply'), 1);
  f.receipt('g', 'x', 1);
  assert.equal(f.sent.length, 1);
});
test('ordinary refusal discards its unforwarded cancellation intent', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.bridge.send(canvas(1, 'cancel'));
  f.receipt('r', 'o', 1);
  f.receipt('g', 'x', 1);
  assert.equal(f.sent.length, 1);
  assert.equal(f.received[0].failure.status, 'unavailable');
  assert.equal(f.received[0].failure.message, '');
});
test('delayed old cancel refusal cannot affect the newer ordinary request or its cancel intent', () => {
  for (const cancelNewRequest of [false, true]) {
    const f = fixture();
    f.receipt('g', 'o', 1);
    f.receipt('g', 'x', 1);
    f.bridge.send(canvas());
    f.bridge.send(canvas(1, 'cancel'));
    f.deliver(canvas(1, 'context.reply'), 1);
    f.receipt('g', 'o', 2);
    f.bridge.send(canvas(2));
    if (cancelNewRequest) f.bridge.send(canvas(2, 'cancel'));
    assert.equal(f.receipt('r', 'x', 1), true);
    assert.equal(f.received.length, 1);
    assert.equal(f.receipt('g', 'x', 2), true);
    assert.equal(f.sent.length, cancelNewRequest ? 4 : 3);
    if (cancelNewRequest) assert.equal(f.sent[3], framed(canvas(2, 'cancel'), 'x', 2));
    assert.equal(f.deliver(canvas(2, 'context.reply'), 2), true);
    assert.equal(f.received.length, 2);
  }
});
test('bounded control FIFO coalesces kinds and refusal preserves a newer pending value', () => {
  const f = fixture();
  f.receipt('g', 'c', 1);
  f.bridge.send(control('pong', { connected: false }));
  for (const type of ['ready', 'applied', 'pong', 'rejected']) f.bridge.send(control(type));
  f.bridge.send(control('pong', { connected: true }));
  assert.equal(f.receipt('r', 'c', 1), true);
  for (let ticket = 2; ticket <= 5; ticket++) { f.receipt('g', 'c', ticket); f.receipt('a', 'c', ticket); }
  assert.deepEqual(f.sent.slice(1).map(value => JSON.parse(value.slice(value.indexOf(':', 4) + 1))),
    [control('ready'), control('applied'), control('pong', { connected: true }), control('rejected')]);
  assert.equal(f.receipt('g', 'c', 6), true);
  assert.equal(f.sent.length, 5);
});
test('permanent malformed control consumption does not replay or settle the ordinary request', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.receipt('g', 'c', 2);
  const malformed = control('ready', { unexpected: true });
  f.bridge.send(malformed);
  assert.equal(f.receipt('a', 'c', 2), true);
  assert.equal(f.receipt('a', 'c', 2), false);
  assert.equal(f.receipt('g', 'c', 3), true);
  assert.deepEqual(f.sent, [framed(canvas(), 'o', 1), framed(malformed, 'c', 2)]);
  assert.deepEqual(f.received, []);
  assert.equal(f.receipt('g', 'o', 4), false);
  f.bridge.send(control());
  assert.equal(f.sent[2], framed(control(), 'c', 3));
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), true);
});
test('permanent malformed cancel consumption permits later valid cancellation without replay', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.receipt('g', 'x', 2);
  const malformed = canvas(1, 'cancel', { unexpected: true });
  f.bridge.send(malformed);
  assert.equal(f.receipt('a', 'x', 2), true);
  assert.equal(f.receipt('g', 'x', 3), true);
  assert.deepEqual(f.sent, [framed(canvas(), 'o', 1), framed(malformed, 'x', 2)]);
  assert.deepEqual(f.received, []);
  assert.equal(f.receipt('g', 'o', 4), false);
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.sent[2], framed(canvas(1, 'cancel'), 'x', 3));
  assert.equal(f.receipt('a', 'x', 3), true);
  assert.equal(f.receipt('g', 'x', 4), true);
  assert.equal(f.sent.length, 3);
  assert.deepEqual(f.received, []);
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), true);
});
for (const disposition of ['a', 'r']) {
  test(`different valid cancel queued behind malformed same-ID cancel survives ${disposition}`, () => {
    const f = fixture();
    f.receipt('g', 'o', 1);
    f.bridge.send(canvas());
    f.receipt('g', 'x', 2);
    const malformed = canvas(1, 'cancel', { unexpected: true });
    f.bridge.send(malformed);
    f.bridge.send(canvas(1, 'cancel'));
    f.bridge.send(canvas(1, 'cancel'));
    assert.equal(f.sent.length, 2);
    assert.equal(f.receipt(disposition, 'x', 2), true);
    assert.equal(f.receipt('g', 'x', 3), true);
    assert.equal(f.sent[2], framed(canvas(1, 'cancel'), 'x', 3));
    assert.deepEqual(f.received, []);
    assert.equal(f.receipt('g', 'o', 4), false);
    const terminal = canvas(1, 'error.reply', { failure: { status: 'cancelled', message: '' } });
    assert.equal(f.deliver(terminal, 1), true);
    assert.deepEqual(f.received, [terminal]);
    f.receipt('a', 'x', 3);
    f.receipt('g', 'x', 4);
    assert.equal(f.sent.length, 3);
  });
}
test('original terminal discards a differing same-ID cancellation waiting for its next grant', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.receipt('g', 'x', 2);
  f.bridge.send(canvas(1, 'cancel', { unexpected: true }));
  f.bridge.send(canvas(1, 'cancel'));
  f.receipt('a', 'x', 2);
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), true);
  assert.equal(f.receipt('g', 'x', 3), true);
  assert.equal(f.sent.length, 2);
  assert.equal(f.received.length, 1);
});
test('retryable overflow refusal preserves unconsumed control and cancel intents', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.receipt('g', 'c', 2);
  f.bridge.send(control());
  f.receipt('g', 'x', 3);
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.receipt('r', 'c', 2), true);
  assert.equal(f.receipt('r', 'x', 3), true);
  assert.deepEqual(f.received, []);
  assert.equal(f.receipt('g', 'o', 4), false);
  assert.equal(f.receipt('g', 'c', 4), true);
  assert.equal(f.receipt('g', 'x', 5), true);
  assert.deepEqual(f.sent.slice(3), [framed(control(), 'c', 4), framed(canvas(1, 'cancel'), 'x', 5)]);
  assert.equal(f.receipt('a', 'c', 4), true);
  assert.equal(f.receipt('a', 'x', 5), true);
  f.receipt('g', 'c', 6);
  f.receipt('g', 'x', 7);
  assert.equal(f.sent.length, 5);
  assert.deepEqual(f.received, []);
});
test('ordinary refusal publishes one existing failure and later credit admits the next request', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  f.bridge.send(canvas(2));
  assert.equal(f.receipt('r', 'o', 1), true);
  assert.equal(f.received[0].failure.status, 'unavailable');
  assert.equal(f.received[0].failure.message, '');
  assert.equal(f.receipt('r', 'o', 1), false);
  assert.equal(f.deliver(canvas(1, 'context.reply'), 1), false);
  assert.equal(f.receipt('g', 'o', 2), true);
  assert.equal(f.sent[1], framed(canvas(2), 'o', 2));
});
test('overflow revokes exact idle grants without public replies and preserves later queued work', () => {
  const f = fixture();
  for (const [lane, ticket] of [['o', 1], ['c', 2], ['x', 3]]) {
    assert.equal(f.receipt('g', lane, ticket), true);
    assert.equal(f.receipt('r', lane, ticket + 100), false);
    assert.equal(f.receipt('r', lane, ticket), true);
    assert.equal(f.receipt('r', lane, ticket), false);
    assert.equal(f.receipt('g', lane, ticket), false);
  }
  assert.deepEqual(f.received, []);
  f.bridge.send(canvas());
  f.bridge.send(control());
  assert.deepEqual(f.sent, []);
  assert.equal(f.receipt('g', 'o', 4), true);
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.sent.length, 1);
  assert.equal(f.receipt('g', 'c', 5), true);
  assert.equal(f.receipt('g', 'x', 6), true);
  assert.deepEqual(f.sent, [framed(canvas(), 'o', 4), framed(control(), 'c', 5), framed(canvas(1, 'cancel'), 'x', 6)]);
  assert.deepEqual(f.received, []);
});
test('permanent consumption retires exact idle control and cancel credits without touching ordinary custody', () => {
  const f = fixture();
  for (const [lane, ticket] of [['c', 1], ['x', 2]]) {
    assert.equal(f.receipt('g', lane, ticket), true);
    assert.equal(f.receipt('a', lane, ticket + 100), false);
    assert.equal(f.receipt('a', lane, ticket), true);
    assert.equal(f.receipt('a', lane, ticket), false);
    assert.equal(f.receipt('r', lane, ticket), false);
    assert.equal(f.receipt('g', lane, ticket), false);
  }
  assert.equal(f.receipt('g', 'o', 3), true);
  assert.equal(f.receipt('a', 'o', 3), false);
  assert.deepEqual(f.received, []);
  f.bridge.send(control());
  assert.deepEqual(f.sent, []);
  f.bridge.send(canvas());
  assert.equal(f.sent[0], framed(canvas(), 'o', 3));
  f.bridge.send(canvas(1, 'cancel'));
  assert.equal(f.sent.length, 1);
  assert.equal(f.receipt('g', 'c', 4), true);
  assert.equal(f.receipt('g', 'x', 5), true);
  assert.deepEqual(f.sent.slice(1), [framed(control(), 'c', 4), framed(canvas(1, 'cancel'), 'x', 5)]);
  assert.deepEqual(f.received, []);
});
test('foreign replies, tickets and invalid receipts cannot release capacity', () => {
  const f = fixture();
  f.receipt('g', 'o', 1);
  f.bridge.send(canvas());
  for (const value of [canvas(2, 'context.reply'), canvas(1, 'surface.reply'), canvas(1, 'context.reply', { version: 2 }), canvas(1, 'context.reply', { session: 'another-session' }), control('configure')])
    assert.equal(f.deliver(value, 1), false);
  assert.equal(f.deliver(canvas(1, 'context.reply'), 2), false);
  for (const args of [['g', '__proto__', 1], ['g', 'x', 0], ['g', 'x', 1.5], ['g', 'x', maxTicket + 1], ['a', 'o', 1], ['r', 'o', 2]])
    assert.equal(f.receipt(...args), false);
  assert.equal(f.received.length, 0);
  assert.equal(f.receipt('g', 'o', 2), false);
});
for (const action of ['reply', 'refusal']) {
  test(`${action} listener failure propagates without granting new capacity`, () => {
    const f = fixture();
    f.receipt('g', 'o', 1);
    f.bridge.send(canvas());
    f.bridge.subscribe(() => { throw new Error('listener failed'); });
    assert.throws(() => action === 'reply' ? f.deliver(canvas(1, 'context.reply'), 1) : f.receipt('r', 'o', 1), /listener failed/);
    assert.equal(f.receipt('g', 'o', 2), false);
    assert.equal(f.sent.length, 1);
  });
}
test('binding exceptions propagate synchronously and queued admission fails closed', () => {
  const direct = fixture(() => { throw new Error('binding failed'); });
  direct.receipt('g', 'o', 1);
  assert.throws(() => direct.bridge.send(canvas()), error => {
    transportError(error, 'binding');
    assert.equal(error.cause.cause.message, 'binding failed');
    return true;
  });
  assert.equal(direct.receipt('g', 'o', 2), false);
  const queued = fixture(() => { throw new Error('binding failed'); });
  queued.bridge.send(canvas());
  assert.equal(queued.receipt('g', 'o', 1), false);
  assert.equal(queued.received[0].failure.status, 'unavailable');
  assert.throws(() => queued.bridge.send(canvas(2)), error => transportError(error, 'closed'));
});
test('unsupported messages reject before custody and leave valid requests usable', () => {
  const f = fixture();
  for (const value of [canvas(1, 'sources.request'), canvas(1, 'masks.request'), canvas(1, 'image.insert'), canvas(1, 'context.request', { version: 2 }), canvas(1, 'context.request', { session: 'another-session' }), canvas(0), control('configure')])
    assert.throws(() => f.bridge.send(value), error => transportError(error, 'unsupported'));
  f.bridge.send(canvas());
  f.receipt('g', 'o', 1);
  assert.equal(f.sent.length, 1);
});
test('literal delivery, subscriptions and page retirement retain their exact lifetime', () => {
  const f = fixture(), reply = control('configure', { message: '\\"\0漢🙂\u2028\u2029</script>);throw Error("injected");//' });
  assert.equal(f.deliver(reply), true);
  assert.deepEqual(f.received, [reply]);
  const remove = f.bridge.subscribe(() => { throw new Error('unsubscribed'); });
  remove();
  f.bridge.send(canvas());
  f.bridge.send(control());
  f.events.get('pagehide')();
  assert.equal(f.deliver(reply), false);
  assert.equal(f.receipt('g', 'o', 1), false);
  assert.throws(() => f.bridge.send(canvas()), error => transportError(error, 'closed'));
  assert.throws(() => f.bridge.send(control()), error => transportError(error, 'closed'));
  assert.doesNotThrow(() => f.bridge.send(canvas(1, 'cancel')));
  assert.throws(() => f.bridge.subscribe(() => {}), error => transportError(error, 'subscription'));
  assert.equal(f.received.length, 1);
  assert.equal(f.sent.length, 0);
});

const watch = (requestId = 1, enabled = true) =>
  canvas(requestId, 'surface.presentation.request', { schema: 1, enabled });
const previewRequest = (step, requestId = 1, data = Buffer.alloc(8192).toString('base64')) => {
  const extra = step === 'begin' ? {
    preview: { generation: 1, promptId: 'prompt:one', sourceId: 'source:one',
      runOrder: 1, sequence: 1, token: 'preview:one' }, byteLength: 8192, format: 'jpeg',
  } : { token: 'preview:one', leaseRevision: 1, ...(step === 'chunk' ? { offset: 0, data } : {}) };
  return canvas(requestId, `surface.preview.${step}`, { address, revision: 1, ...extra });
};
const presentation = () => canvas(0, 'surface.presentation.changed', { schema: 1, change: {
  address, sequence: 1, token: 'preview:one', leaseRevision: 1, status: 'presented',
  displayed: { type: 'preview', token: 'preview:one', leaseRevision: 1,
    viewport: viewportReply().viewport }, message: '',
} });

for (const scripts of [sources, [...sources].reverse()]) {
  test(`bootstrap installs once with ${scripts === sources ? 'policy' : 'transport'} evaluated first`, () => {
    const f = fixture(undefined, scripts), bridge = f.bridge;
    for (let count = 0; count < 2; count++)
      new Script('globalThis.__sugarcubes_substitute_install_v3()').runInContext(f.context);
    assert.equal(f.context.sugarcubesPanelHost, bridge);
    assert.equal(f.events.size, 1);
    f.receipt('g', 'o', 1);
    bridge.send(watch());
    assert.deepEqual(f.sent, [framed(watch(), 'o', 1)]);
    f.events.get('pagehide')();
    new Script('globalThis.__sugarcubes_substitute_install_v3()').runInContext(f.context);
    assert.throws(() => bridge.send(watch(2)), error => transportError(error, 'closed'));
  });
}

test('either missing bootstrap keeps the public bridge absent', () => {
  for (const script of sources) {
    const f = fixture(undefined, [script]);
    assert.equal(f.bridge, undefined);
    assert.equal(f.receipt, undefined);
    assert.equal(f.events.size, 0);
  }
});

test('presentation watch requires its named reply and retires only on the native ticket', () => {
  const request = watch(), f = admit(request);
  assert.deepEqual(f.sent, [framed(request)]);
  for (const type of ['surface.reply', 'surface.presentation.changed', 'surface.capabilities.reply'])
    assert.equal(f.deliver(canvas(1, type), maxTicket), false);
  assert.equal(f.deliver(canvas(1, 'surface.presentation.reply'), maxTicket), true);
  assert.equal(f.deliver(canvas(1, 'surface.presentation.reply'), maxTicket), false);
});

for (const step of ['begin', 'chunk', 'end', 'withdraw']) {
  test(`preview ${step} preserves ordinary custody and accepts only the shared surface reply`, () => {
    const request = previewRequest(step), f = admit(request);
    assert.deepEqual(f.sent, [framed(request)]);
    for (const type of [`surface.preview.reply`, `surface.preview.${step}.reply`, 'surface.presentation.reply'])
      assert.equal(f.deliver(canvas(1, type), maxTicket), false);
    const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: '' });
    assert.equal(f.deliver(reply, maxTicket), true);
    assert.equal(f.deliver(reply, maxTicket), false);
  });

  test(`queued preview ${step} cancellation keeps its addressed terminal reply`, () => {
    const f = fixture();
    f.bridge.send(previewRequest(step));
    f.bridge.send(canvas(1, 'cancel'));
    assert.deepEqual(f.sent, []);
    assert.deepEqual(f.received, [canvas(1, 'surface.reply', {
      address, revision: 1, status: 'cancelled', message: '',
    })]);
    f.receipt('g', 'o', 1);
    f.bridge.send(viewport(2));
    assert.deepEqual(f.sent, [framed(viewport(2), 'o', 1)]);
  });
}

test('queued watch cancellation emits the address-free shared error shape', () => {
  const f = fixture();
  f.bridge.send(watch());
  f.bridge.send(canvas(1, 'cancel'));
  assert.deepEqual(f.sent, []);
  assert.deepEqual(f.received, [canvas(1, 'error.reply', { failure: { status: 'cancelled', message: '' } })]);
  f.bridge.send(watch(2, false));
  f.receipt('g', 'o', 1);
  assert.deepEqual(f.sent, [framed(watch(2, false), 'o', 1)]);
});

for (const request of [watch(), previewRequest('chunk')]) {
  test(`forwarded ${request.type} cancellation waits for its actual ordinary terminal`, () => {
    const f = fixture();
    f.receipt('g', 'o', 1);
    f.receipt('g', 'x', 1);
    f.bridge.send(request);
    f.bridge.send(canvas(1, 'cancel'));
    assert.equal(f.receipt('a', 'x', 1), true);
    assert.deepEqual(f.received, []);
    assert.equal(f.receipt('g', 'o', 2), false);
    const reply = canvas(1, 'error.reply', { failure: { status: 'cancelled', message: '' } });
    assert.equal(f.deliver(reply, 1), true);
    assert.equal(f.receipt('g', 'o', 2), true);
  });
}

test('maximum preview chunk fits both public bounds and oversized encoded bytes travel through the bulk channel', async () => {
  const transfer = bulkFetch(), request = previewRequest('chunk'), f = fixture(undefined, sources, transfer.fetch);
  assert.equal(request.data.length, 10924);
  f.receipt('g', 'o', 1);
  f.bridge.send(request);
  assert.equal(f.sent[0], framed(request, 'o', 1));
  assert.ok(Buffer.byteLength(JSON.stringify(request)) <= 16384);
  assert.ok(envelope(f.sent[0]).length + 1 <= 16384);
  const oversized = previewRequest('chunk', 2, Buffer.alloc(16384).toString('base64'));
  f.bridge.send(oversized);
  assert.equal(f.sent.length, 1);
  const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: '' });
  assert.equal(f.deliver(reply, 1), true);
  assert.equal(f.receipt('g', 'o', 2, grant), true);
  await settle();
  assert.equal(transfer.requests[0].init.body, JSON.stringify(oversized));
  assert.equal(f.sent[1], `1:o:2:${bulkEnvelope(grant, Buffer.byteLength(JSON.stringify(oversized)))}`);
});
test('presentation notifications interleave preview bytes and final viewport without releasing custody', () => {
  const f = fixture(), notification = presentation();
  f.receipt('g', 'o', 1);
  f.bridge.send(previewRequest('chunk'));
  f.bridge.send(viewport(2));
  assert.equal(f.deliver(notification), true);
  assert.deepEqual(f.received, [notification]);
  assert.equal(f.receipt('g', 'o', 2), false);
  assert.equal(f.sent.length, 1);
  assert.equal(f.deliver(notification, 1), false);
  assert.equal(f.deliver(canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: '' }), 1), true);
  assert.equal(f.receipt('g', 'o', 2), true);
  assert.equal(f.sent[1], framed(viewport(2), 'o', 2));
  assert.equal(f.deliver(notification), true);
  assert.equal(f.receipt('g', 'o', 3), false);
  assert.equal(f.deliver(viewportReply(2), 2), true);
  assert.equal(f.receipt('g', 'o', 3), true);
});

test('unsolicited canvas delivery admits only exact current-session zero-id presentation notifications', () => {
  const f = fixture(), notification = presentation();
  f.receipt('g', 'o', 1);
  for (const value of [canvas(0, 'surface.reply'), canvas(0, 'surface.presentation.reply'),
    canvas(0, 'context.reply'), canvas(1, 'surface.presentation.changed'),
    { ...notification, requestId: undefined }, { ...notification, version: 2 },
    { ...notification, session: 'another-session' }, { ...notification, protocol: 'other' }])
    assert.equal(f.deliver(value), false);
  assert.deepEqual(f.received, []);
  assert.equal(f.deliver(notification), true);
  f.bridge.send(watch());
  assert.deepEqual(f.sent, [framed(watch(), 'o', 1)]);
  assert.equal(f.receipt('g', 'o', 2), false);
});
test('settled camera notifications share the zero-id presentation admission', () => {
  const f = fixture(), camera = canvas(0, 'surface.camera.changed', { schema: 1, change: {
    address, sequence: 2, selectionRevision: 1, viewport: viewportReply().viewport,
  } });
  f.receipt('g', 'o', 1);
  for (const value of [canvas(1, 'surface.camera.changed'), { ...camera, session: 'another-session' },
    { ...camera, version: 2 }])
    assert.equal(f.deliver(value), false);
  assert.equal(f.deliver(camera), true);
});
const nativeRequests = ['source.request', 'geometry.request', 'edit.request', 'workflow.request'];
for (const type of nativeRequests) {
  test(`native ${type} rides the ordinary lane and accepts only its own reply or an error`, () => {
    const f = fixture(), request = canvas(1, type, { command: { type: 'context' } });
    const reply = type.replace(/request$/, 'reply');
    f.bridge.send(request);
    assert.equal(f.receipt('g', 'o', 1), true);
    assert.deepEqual(f.sent, [framed(request, 'o', 1)]);
    for (const other of nativeRequests.filter(value => value !== type))
      assert.equal(f.deliver(canvas(1, other.replace(/request$/, 'reply')), 1), false);
    assert.equal(f.deliver(canvas(2, reply), 1), false);
    assert.equal(f.deliver(canvas(1, reply, { result: { kind: 'failure', status: 'unsupported', message: '' } }), 1), true);
    assert.deepEqual(f.received.map(value => value.type), [reply]);
    const next = admit(canvas(3, type, { command: { type: 'context' } }));
    assert.equal(next.deliver(canvas(3, 'error.reply', { failure: { status: 'cancelled', message: '' } }), maxTicket), true);
  });
}
test('source and edit change notifications are zero-id current-session notifications', () => {
  const f = fixture();
  const changes = [canvas(0, 'source.changed', { change: { documentId: null, revision: 'r' } }),
    canvas(0, 'edit.changed', { state: { revision: 1 } })];
  for (const change of changes) {
    for (const value of [{ ...change, requestId: 1 }, { ...change, session: 'another-session' },
      { ...change, version: 2 }, { ...change, protocol: 'other' }])
      assert.equal(f.deliver(value), false);
    assert.equal(f.deliver(change), true);
  }
  assert.deepEqual(f.received.map(value => value.type), ['source.changed', 'edit.changed']);
  for (const type of ['workflow.changed', 'geometry.changed'])
    assert.equal(f.deliver(canvas(0, type)), false);
});
test('every shared surface and native canvas request type is admitted by the bridge policy', () => {
  const schema = JSON.parse(readFileSync(
    new URL('../../../../Sugar/integrations/contracts/canvas.schema.json', import.meta.url), 'utf8'));
  const context = createContext({});
  new Script(policySource).runInContext(context);
  const policy = context.__sugarcubes_substitute_policy_v3;
  const sharedTypes = Object.entries(schema.definitions ?? schema.$defs ?? {})
    .filter(([name, definition]) => name.endsWith('Request') && definition.properties?.type?.const)
    .map(([, definition]) => definition.properties.type.const)
    // The panel bridge carries every request except layer insertion, which the native host never
    // advertises; cancellation travels the bridge's separate cancel lane.
    .filter(type => !['image.insert', 'cancel'].includes(type));
  assert.ok(sharedTypes.includes('surface.overview') && sharedTypes.includes('surface.select'));
  for (const type of ['context.request', ...nativeRequests]) assert.ok(sharedTypes.includes(type));
  for (const type of sharedTypes) assert.ok(policy.request(type), `${type} is not admitted`);
});

test('oversized ordinary request uploads under its credit grant and sends only the envelope', async () => {
  const transfer = bulkFetch(), f = fixture(undefined, sources, transfer.fetch);
  const request = selection('x'.repeat(20 * 1024 * 1024)), encoded = JSON.stringify(request);
  assert.equal(f.receipt('g', 'o', 3, grant), true);
  f.bridge.send(request);
  assert.deepEqual(f.sent, []);
  await settle();
  const [upload] = transfer.requests;
  assert.equal(upload.url, `${bulkBase}u/${grant}`);
  assert.equal(upload.init.method, 'POST');
  assert.equal(upload.init.body, encoded);
  for (const [key, value] of Object.entries({ mode: 'same-origin', credentials: 'omit', cache: 'no-store',
    redirect: 'error', referrerPolicy: 'no-referrer' })) assert.equal(upload.init[key], value);
  assert.equal(upload.init.signal.aborted, false);
  assert.deepEqual(f.sent, [`1:o:3:${bulkEnvelope(grant, Buffer.byteLength(encoded))}`]);
  assert.ok(f.sent[0].length < 256);
  assert.equal(f.deliver(canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: '' }), 3), true);
  assert.equal(f.receipt('g', 'o', 4, 'cd'.repeat(16)), true);
});
test('a failed or ungranted upload still sends its envelope so the host refuses the credit', async () => {
  const failing = bulkFetch(() => { throw new TypeError('network'); });
  const f = fixture(undefined, sources, failing.fetch), request = selection('x'.repeat(20000));
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(request);
  await settle();
  assert.deepEqual(f.sent, [`1:o:1:${bulkEnvelope(grant, Buffer.byteLength(JSON.stringify(request)))}`]);
  assert.equal(f.receipt('r', 'o', 1), true);
  assert.deepEqual(f.received, [canvas(1, 'error.reply', { failure: { status: 'unavailable', message: '' } })]);
  const ungranted = bulkFetch(), g = fixture(undefined, sources, ungranted.fetch);
  g.receipt('g', 'o', 1);
  g.bridge.send(request);
  await settle();
  assert.equal(ungranted.requests.length, 0);
  assert.deepEqual(g.sent, [`1:o:1:${JSON.stringify({ transport: 'sugarsubstitute.bulk', version: 1, bytes: Buffer.byteLength(JSON.stringify(request)) })}`]);
});
test('oversized control and messages beyond the bulk bound refuse before custody', () => {
  const f = fixture(undefined, sources, bulkFetch().fetch);
  f.receipt('g', 'c', 1);
  f.receipt('g', 'o', 1, grant);
  assert.throws(() => f.bridge.send(control('rejected', { message: 'x'.repeat(20000) })), error => transportError(error, 'bound'));
  assert.throws(() => f.bridge.send(selection('x'.repeat(100663296))), error => transportError(error, 'bound'));
  assert.deepEqual(f.sent, []);
  f.bridge.send(control());
  assert.deepEqual(f.sent, [framed(control(), 'c', 1)]);
});
test('bulk reply downloads exact bytes, validates like inline and preserves host order', async () => {
  const reply = canvas(1, 'context.reply', { message: 'r'.repeat(40 * 1024 * 1024) }), text = JSON.stringify(reply);
  const transfer = bulkFetch(url => url === `${bulkBase}d/${grant}` ? body(text) : body('', 404));
  const f = fixture(undefined, sources, transfer.fetch), notification = control('configure');
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(canvas());
  assert.equal(f.deliver(JSON.parse(bulkEnvelope(grant, Buffer.byteLength(text))), 1), true);
  assert.equal(f.deliver(notification), true);
  assert.deepEqual(f.received, []);
  assert.equal(f.receipt('g', 'o', 2, 'cd'.repeat(16)), true, 'the delivered envelope settled the credit');
  await settle();
  assert.equal(transfer.requests[0].init.method, 'GET');
  assert.equal(f.received.length, 2);
  assert.equal(f.received[0].message.length, reply.message.length);
  assert.deepEqual(f.received[1], notification);
});
test('a bulk reply failing download, size or reply validation settles as unavailable', async () => {
  const wrong = JSON.stringify(canvas(1, 'context.reply', { session: 'another-session' }));
  for (const respond of [() => body('', 404), () => body('{"x":1}', 200), () => body(wrong)]) {
    const f = fixture(undefined, sources, bulkFetch(respond).fetch);
    f.receipt('g', 'o', 1, grant);
    f.bridge.send(canvas());
    assert.equal(f.deliver(JSON.parse(bulkEnvelope(grant, Buffer.byteLength(wrong))), 1), true);
    await settle();
    assert.deepEqual(f.received, [canvas(1, 'error.reply', { failure: { status: 'unavailable', message: '' } })]);
  }
});
test('bulk notifications validate as inline notifications and invalid ones are dropped', async () => {
  const valid = JSON.stringify(presentation()), foreign = JSON.stringify({ ...presentation(), session: 'another-session' });
  const transfer = bulkFetch(url => body(url.endsWith('cd'.repeat(16)) ? foreign : valid));
  const f = fixture(undefined, sources, transfer.fetch);
  assert.equal(f.deliver(JSON.parse(bulkEnvelope('cd'.repeat(16), Buffer.byteLength(foreign)))), true);
  assert.equal(f.deliver(JSON.parse(bulkEnvelope(grant, Buffer.byteLength(valid)))), true);
  await settle();
  assert.deepEqual(f.received, [presentation()]);
});
test('malformed bulk envelopes are not deliveries', () => {
  const f = fixture(undefined, sources, bulkFetch().fetch);
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(canvas());
  const valid = JSON.parse(bulkEnvelope(grant, 10));
  for (const value of [{ ...valid, extra: 1 }, { ...valid, token: grant.toUpperCase() }, { ...valid, bytes: 0 },
    { ...valid, bytes: 100663297 }, { ...valid, version: 2 }, { ...valid, transport: 'other' }]) {
    assert.equal(f.deliver(value, 1), false);
    assert.equal(f.deliver(value), false);
  }
  assert.equal(f.deliver(valid, 2), false, 'a foreign ticket cannot settle the active request');
});
test('pagehide aborts in-flight transfers and drops pending deliveries', async () => {
  const signals = [];
  const pending = (url, init) => { signals.push(init.signal); return new Promise(() => {}); };
  const f = fixture(undefined, sources, pending);
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(canvas());
  f.deliver(JSON.parse(bulkEnvelope(grant, 10)), 1);
  f.deliver(control('configure'));
  f.events.get('pagehide')();
  assert.equal(signals.length, 1);
  assert.equal(signals[0].aborted, true);
  await settle();
  assert.deepEqual(f.received, []);
});test('bulk URLs join the document origin captured at start to the host-supplied reserved path', async () => {
  const remote = 'https://proxy.example', path = '/comfy/__sugarsubstitute/bulk/';
  const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: 'r'.repeat(20000) });
  const text = JSON.stringify(reply);
  const transfer = bulkFetch((url, init) => init.method === 'GET' ? body(text) : { status: 204 });
  const scripts = [policySource, bulkFor(path), source];
  const f = fixture(undefined, scripts, transfer.fetch, remote);
  f.context.location.origin = 'https://elsewhere.example';
  f.receipt('g', 'o', 1, grant);
  f.bridge.send(selection('x'.repeat(20000)));
  await settle();
  assert.equal(f.deliver(JSON.parse(bulkEnvelope('cd'.repeat(16), Buffer.byteLength(text))), 1), true);
  await settle();
  assert.deepEqual(transfer.requests.map(({ url, init }) => [init.method, url, init.mode]), [
    ['POST', `${remote}${path}u/${grant}`, 'same-origin'],
    ['GET', `${remote}${path}d/${'cd'.repeat(16)}`, 'same-origin'],
  ]);
  assert.equal(f.received[0].message.length, reply.message.length);
  assert.ok(!bulkSource.includes('.invalid'));
});
