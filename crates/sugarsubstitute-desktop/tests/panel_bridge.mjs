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
// Match the host's composition: the protocol expression receives the output surface preference.
const policyFor = preference => `${policySource.trimEnd()}(${preference});\n`;
const sources = [policyFor("'host'"), source];
const windowedSources = [policyFor('undefined'), source];
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
function fixture(post, scripts = sources) {
  const sent = [], received = [], events = new Map(), window = {};
  window.top = window;
  const context = createContext({ window, URLSearchParams, TextEncoder,
    location: { search: `?sc-panel-host=${host}&sc-panel-session=${session}&sc-panel-transport=native` },
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
function admit(message, lane = 'o') {
  const f = fixture();
  assert.equal(f.receipt('g', lane, maxTicket), true);
  f.bridge.send(message);
  return f;
}

test('each bootstrap script fits the unchanged native registration command', () => {
  for (const source of [...sources, ...windowedSources])
    assert.ok(Buffer.byteLength(JSON.stringify({ source })) <= 8192);
});
test('only a composition-hosted browser advertises host output presentation', () => {
  assert.equal(fixture().bridge.outputSurfacePreference, 'host');
  const windowed = fixture(undefined, windowedSources).bridge;
  assert.ok(windowed);
  assert.equal(Object.hasOwn(windowed, 'outputSurfacePreference'), false);
  assert.equal(windowed.preferencePolicy, 'saved');
});
test('escaped selection refuses before custody and permits a later small request', () => {
  const f = fixture(), request = selection('\\'.repeat(3800));
  request.file.subfolder = '\\'.repeat(3800);
  assert.ok(Buffer.byteLength(JSON.stringify(request)) <= 16384);
  assert.ok(envelope(framed(request)).length + 1 > 16384);
  assert.throws(() => f.bridge.send(request), error => transportError(error, 'bound'));
  assert.deepEqual(f.sent, []);
  f.receipt('g', 'c', 1);
  f.bridge.send(control());
  assert.deepEqual(f.sent, [framed(control(), 'c', 1)]);
});
test('UTF-16 boundary includes private prefix, terminator and maximum context identity', () => {
  const overhead = envelope(framed(selection())).length + 1;
  const exact = selection('x'.repeat(16384 - overhead)), f = admit(exact);
  assert.equal(envelope(f.sent[0]).length + 1, 16384);
  assert.throws(() => f.bridge.send(selection(`${exact.file.filename}x`)), error => transportError(error, 'bound'));
  assert.equal(f.sent.length, 1);
});
// Well-formed JSON has UTF16 units <= UTF8 bytes <= 3 * UTF16 units.
// ASCII framing/escaping preserves bytes minus units, so these two 16384
// bounds imply native UTF8 <= 27305, within its independent 32768-byte limit.
for (const character of ['\\', '"', '\0', '\n', '漢', '🙂', '\u2028', '\u2029', '漢\\🙂"\n', '\ud800漢\u07ff']) {
  test(`admission preserves ${JSON.stringify(character)} at the first native limit`, () => {
    let first = 0, last = 20000;
    while (first < last) {
      const middle = Math.ceil((first + last) / 2);
      try { admit(selection(character.repeat(middle))); first = middle; }
      catch (error) { transportError(error, 'bound'); last = middle - 1; }
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
    assert.throws(() => f.bridge.send(selection(character.repeat(first + 1))), error => transportError(error, 'bound'));
    assert.equal(f.sent.length, 1);
  });
}
test('control grant is independent and configure can start context before applied', () => {
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
  for (const value of [canvas(1, 'sources.request'), canvas(1, 'geometry.request'), canvas(1, 'image.insert'), canvas(1, 'context.request', { version: 2 }), canvas(1, 'context.request', { session: 'another-session' }), canvas(0), control('configure')])
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

test('maximum preview chunk fits both public bounds and oversized encoded bytes refuse before custody', () => {
  const request = previewRequest('chunk'), f = fixture();
  assert.equal(request.data.length, 10924);
  f.receipt('g', 'o', 1);
  f.bridge.send(request);
  assert.equal(f.sent[0], framed(request, 'o', 1));
  assert.ok(Buffer.byteLength(JSON.stringify(request)) <= 16384);
  assert.ok(envelope(f.sent[0]).length + 1 <= 16384);
  const oversized = previewRequest('chunk', 2, Buffer.alloc(16384).toString('base64'));
  assert.throws(() => f.bridge.send(oversized), error => transportError(error, 'bound'));
  assert.equal(f.sent.length, 1);
  const reply = canvas(1, 'surface.reply', { address, revision: 1, status: 'applied', message: '' });
  assert.equal(f.deliver(reply, 1), true);
  f.receipt('g', 'o', 2);
  f.bridge.send(viewport(2));
  assert.equal(f.sent[1], framed(viewport(2), 'o', 2));
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
test('every shared surface and context request type is admitted by the bridge policy', () => {
  const schema = JSON.parse(readFileSync(
    new URL('../../../third_party/sugarcubes-canvas/canvas.schema.json', import.meta.url), 'utf8'));
  const context = createContext({});
  new Script(policyFor("'host'")).runInContext(context);
  const policy = context.__sugarcubes_substitute_policy_v3;
  const sharedTypes = Object.entries(schema.definitions ?? schema.$defs ?? {})
    .filter(([name, definition]) => name.endsWith('Request') && definition.properties?.type?.const)
    .map(([, definition]) => definition.properties.type.const)
    // The panel bridge carries surface and context requests; editor requests use other hosts,
    // and cancellation travels the bridge's separate cancel lane.
    .filter(type => type.startsWith('surface.') || type === 'context.request');
  assert.ok(sharedTypes.includes('surface.overview') && sharedTypes.includes('surface.select'));
  for (const type of sharedTypes) assert.ok(policy.request(type), `${type} is not admitted`);
});