// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Execute the actual injected transport with Node's built-in test runner.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { Script, createContext } from 'node:vm';

const source = readFileSync(
  process.argv[2] ?? new URL('../src/main_actor/panel/bridge.js', import.meta.url), 'utf8',
);
const binding = '__sugarcubes_substitute_send_v3';
const host = 'ai.artificialsweetener.sugarsubstitute';
const session = 'session-12345678';

/** Install production behavior while faking only its browser and native-call boundaries. */
function fixture() {
  const sent = [];
  const events = new Map();
  const window = {};
  window.top = window;
  const context = createContext({
    window, URLSearchParams, TextEncoder,
    location: { search: `?sc-panel-host=${host}&sc-panel-session=${session}&sc-panel-transport=native` },
    [binding]: (payload) => sent.push(payload),
    addEventListener: (name, listener) => events.set(name, listener),
  });
  new Script(source).runInContext(context);
  return { bridge: context.sugarcubesPanelHost, context, sent, events };
}

/** Represent the complete native event with the longest signed context ID and its terminator. */
function envelope(payload) {
  return JSON.stringify({ name: binding, payload, executionContextId: 0 })
    .replace('"executionContextId":0', '"executionContextId":-9223372036854775808');
}

test('the actual injected script fits the native registration command', () => {
  assert.ok(Buffer.byteLength(JSON.stringify({ source })) <= 8192);
});

test('schema-shaped escaped selection refuses before native send and leaves the bridge usable', () => {
  const f = fixture();
  const request = {
    protocol: 'sugarcubes.canvas', version: 3, session, requestId: 1,
    type: 'surface.select', revision: 1,
    address: { sessionId: 'canvas:w', viewId: 'view:a', windowId: 'window:a',
      attachmentId: 'attachment:a', generation: 1 },
    itemId: 'result:one', file: { filename: '\\'.repeat(3800), subfolder: '\\'.repeat(3800), type: 'output' },
  };
  const payload = JSON.stringify(request);
  assert.ok(Buffer.byteLength(payload) <= 16384);
  assert.ok(envelope(payload).length + 1 > 16384);
  assert.throws(() => f.bridge.send(request), /transport bound/);
  assert.deepEqual(f.sent, []);
  const ready = { protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' };
  f.bridge.send(ready);
  assert.deepEqual(f.sent, [JSON.stringify(ready)]);
});

test('the native UTF-16 boundary includes its terminator and maximum context identity', () => {
  const f = fixture();
  const overhead = envelope(JSON.stringify({ value: '' })).length + 1;
  const exact = { value: 'x'.repeat(16384 - overhead) };
  f.bridge.send(exact);
  assert.equal(envelope(f.sent[0]).length + 1, 16384);
  assert.throws(() => f.bridge.send({ value: `${exact.value}x` }), /transport bound/);
  assert.equal(f.sent.length, 1);
});

for (const character of ['\\', '"', '\0', '\n', '漢', '🙂', '\u2028', '\u2029']) {
  test(`admission preserves ${JSON.stringify(character)} and refuses beyond the first native limit`, () => {
    const f = fixture();
    let first = 0;
    let last = 20000;
    while (first < last) {
      const middle = Math.ceil((first + last) / 2);
      try {
        f.bridge.send({ value: character.repeat(middle) });
        first = middle;
      } catch (error) {
        assert.match(String(error), /transport bound/);
        last = middle - 1;
      }
    }
    f.sent.length = 0;
    const exact = { value: character.repeat(first) };
    f.bridge.send(exact);
    assert.equal(f.sent[0], JSON.stringify(exact));
    assert.ok(envelope(f.sent[0]).length + 1 <= 16384);
    assert.ok(Buffer.byteLength(envelope(f.sent[0])) <= 32768);
    assert.throws(() => f.bridge.send({ value: character.repeat(first + 1) }), /transport bound/);
    assert.equal(f.sent.length, 1);
  });
}

test('actual delivery preserves data, subscription lifetime and markup-like strings', () => {
  const f = fixture();
  const received = [];
  f.bridge.subscribe((value) => received.push(value));
  const reply = { message: '\\"\0漢🙂\u2028\u2029</script>);throw Error("injected");//' };
  assert.equal(f.context.__sugarcubes_substitute_deliver_v3(reply), true);
  assert.deepEqual(received, [reply]);
  f.events.get('pagehide')();
  f.context.__sugarcubes_substitute_deliver_v3(reply);
  assert.equal(received.length, 1);
});
