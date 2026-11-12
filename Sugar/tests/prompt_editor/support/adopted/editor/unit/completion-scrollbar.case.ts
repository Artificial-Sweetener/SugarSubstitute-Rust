//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Literal DOM/capture transports characterize the actual adapter; native wheel/overflow geometry remains browser proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  CompletionScrollbar,
  completionThumb,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-scrollbar.ts';
import { CompletionNode as Node } from './completion-dom-fixture.ts';
import type { PreparedCompletionResult } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';

test('Fluent track geometry reserves both native offsets and retires an unusable tiny track', () => {
  assert.deepEqual(completionThumb(620, 1240, 310), {
    length: 295,
    travel: 295,
    top: 162,
  });
  assert.deepEqual(completionThumb(100, 400, 0), {
    length: 30,
    travel: 40,
    top: 15,
  });
  assert.deepEqual(completionThumb(100, 400, 300), {
    length: 30,
    travel: 40,
    top: 55,
  });
  assert.equal(completionThumb(60, 400, 0), null);
  const f = fixture();
  try {
    f.flush();
    pointer(f.thumb, 'pointerdown');
    f.flush();
    f.viewport.clientHeight = 60;
    f.flush();
    assert.equal(f.thumb.hidden, true);
    assert.equal(f.thumb.captures.size, 0);
    const top = f.viewport.scrollTop;
    pointer(f.thumb, 'pointermove', 500);
    assert.equal(f.viewport.scrollTop, top);
  } finally {
    f.scrollbar.destroy();
  }
});

/** Opaque prepared results are only identity tokens at this adapter boundary. */
function result(): PreparedCompletionResult {
  return Object.freeze({}) as PreparedCompletionResult;
}

/** Mount the actual helper with controlled native dimensions and a finite request transport. */
function fixture() {
  const viewport = new Node();
  let queued = 0;
  const scrollbar = new CompletionScrollbar(viewport as unknown as HTMLElement, () => {
    queued++;
  });
  const carrier = scrollbar.dom as unknown as Node;
  viewport.append(carrier);
  const thumb = carrier.children[0]!;
  const value = result(),
    presentation = Symbol();
  scrollbar.session(value, presentation);
  /** Deliver only the real read/project owner phases. */
  function flush() {
    viewport.readsAllowed = true;
    let geometry;
    try {
      geometry = scrollbar.read();
    } finally {
      viewport.readsAllowed = false;
    }
    scrollbar.project(geometry);
    return geometry;
  }
  return {
    viewport,
    scrollbar,
    carrier,
    thumb,
    value,
    presentation,
    flush,
    queued: () => queued,
  };
}

/** Dispatch one literal pointer event without fabricating browser geometry or a scroll renderer. */
function pointer(node: Node, type: string, y = 10): void {
  const event = new Event(type, { cancelable: true });
  Object.defineProperties(event, {
    button: { value: 0 },
    pointerId: { value: 7 },
    clientY: { value: y },
  });
  node.dispatchEvent(event);
}

test('actual thumb projects native scroll state inside the unchanged viewport ancestry', () => {
  const f = fixture();
  f.viewport.scrollTop = 75;
  const geometry = f.flush();
  assert.equal(geometry?.scale, 2);
  assert.equal(f.carrier.parentElement, f.viewport);
  assert.equal(f.thumb.parentElement, f.carrier);
  assert.equal(f.carrier.attributes.get('aria-hidden'), 'true');
  assert.equal(f.thumb.hidden, false);
  assert.equal(f.thumb.style.height, '30px');
  assert.equal(f.thumb.style.top, '25px');
  const wheel = new Event('wheel', { cancelable: true });
  f.thumb.dispatchEvent(wheel);
  assert.equal(wheel.defaultPrevented, false);
  assert.equal(f.viewport.scrollTop, 75);
  f.scrollbar.destroy();
  assert.equal(f.carrier.parentElement, null);
});

test('pointer input consumes fresh READ scale and retains its baseline across native scroll publications', () => {
  const f = fixture();
  f.flush();
  pointer(f.thumb, 'pointerdown');
  pointer(f.thumb, 'pointermove', 80);
  assert.equal(f.viewport.scrollTop, 0);
  f.flush();
  assert.equal(f.viewport.scrollTop, 262.5);
  f.flush();
  pointer(f.thumb, 'pointermove', 150);
  assert.equal(f.viewport.scrollTop, 300);
  pointer(f.thumb, 'pointerup', 150);
  assert.equal(f.thumb.captures.size, 0);
  assert.equal(f.thumb.released, 1);
  f.scrollbar.destroy();
});

test('pointer completion before baseline READ cannot start a retired drag', () => {
  const f = fixture();
  f.flush();
  pointer(f.thumb, 'pointerdown');
  pointer(f.thumb, 'pointermove', 80);
  pointer(f.thumb, 'pointerup', 80);
  f.flush();
  assert.equal(f.viewport.scrollTop, 0);
  assert.equal(f.thumb.captures.size, 0);
  f.scrollbar.destroy();
});

test('replacement and resize retire exact capture and reject the previous measured publication', () => {
  for (const mode of ['replacement', 'resize'] as const) {
    const f = fixture();
    const old = f.flush();
    pointer(f.thumb, 'pointerdown');
    f.flush();
    if (mode === 'replacement') f.scrollbar.session(result(), Symbol());
    else f.scrollbar.invalidate();
    pointer(f.thumb, 'pointermove', 200);
    f.scrollbar.project(old);
    assert.equal(f.viewport.scrollTop, 0);
    assert.equal(f.thumb.captures.size, 0);
    f.flush();
    assert.equal(f.thumb.hidden, false);
    f.scrollbar.destroy();
  }
});

test('no overflow hides the thumb while stale queued work stays inert after disposal', () => {
  const f = fixture();
  const old = f.flush();
  f.viewport.scrollHeight = 100;
  f.flush();
  assert.equal(f.thumb.hidden, true);
  pointer(f.thumb, 'pointerdown');
  assert.equal(f.queued(), 0);
  f.scrollbar.destroy();
  f.scrollbar.project(old);
  assert.equal(f.scrollbar.read(), null);
  assert.equal(f.carrier.removed, 1);
  f.scrollbar.destroy();
  assert.equal(f.carrier.removed, 1);
});

test('read callback replacement rejects geometry before any old output can publish', () => {
  const f = fixture();
  f.flush();
  f.viewport.getBoundingClientRect = () => {
    f.scrollbar.session(result(), Symbol());
    return { height: 200, left: 0, top: 0, width: 200 };
  };
  f.viewport.readsAllowed = true;
  assert.equal(f.scrollbar.read(), null);
  f.viewport.readsAllowed = false;
  f.scrollbar.destroy();
});

test('event scheduling failure preserves raw primary and independent capture/removal cleanup causes', () => {
  const primary = null,
    cleanup = undefined;
  // The external listener transport exposes raw callback failures synchronously.
  const callbacks = new Map<string, EventListener>();
  const secondViewport = new Node();
  const create = secondViewport.ownerDocument.createElement;
  secondViewport.ownerDocument.createElement = () => {
    const node = create();
    node.addEventListener = (type, callback) => {
      if (callback) callbacks.set(type, callback as EventListener);
    };
    return node;
  };
  const second = new CompletionScrollbar(secondViewport as unknown as HTMLElement, () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw primary;
  });
  const secondCarrier = second.dom as unknown as Node;
  secondViewport.append(secondCarrier);
  secondCarrier.removeFailure = { cause: cleanup };
  second.session(result(), Symbol());
  secondViewport.readsAllowed = true;
  const measured = second.read();
  secondViewport.readsAllowed = false;
  second.project(measured);
  const event = new Event('pointerdown', { cancelable: true });
  Object.defineProperties(event, {
    button: { value: 0 },
    pointerId: { value: 7 },
    clientY: { value: 10 },
  });
  assert.throws(
    () => callbacks.get('pointerdown')!(event),
    (error: unknown) => {
      assert.ok(error instanceof AggregateError);
      assert.equal(error.errors[0], primary);
      assert.equal(error.errors[1], cleanup);
      return true;
    },
  );
  assert.equal(secondCarrier.children[0]!.captures.size, 0);
  assert.equal(secondCarrier.removed, 1);
});

for (const raw of [null, undefined])
  test(`terminal removal preserves a lone raw ${raw === null ? 'null' : 'undefined'} cause`, () => {
    const f = fixture();
    f.carrier.removeFailure = { cause: raw };
    let caught = false;
    try {
      f.scrollbar.destroy();
    } catch (error) {
      caught = true;
      assert.equal(error, raw);
    }
    assert.equal(caught, true);
    assert.equal(f.carrier.removed, 1);
    f.scrollbar.destroy();
    assert.equal(f.carrier.removed, 1);
  });

test('partial listener acquisition retires its acquired carrier and preserves the raw constructor failure', () => {
  const viewport = new Node(),
    raw = Object.freeze({ code: 1 });
  let carrier: Node | null = null,
    created = 0;
  viewport.ownerDocument.createElement = () => {
    const node = new Node();
    created++;
    if (created === 1) carrier = node;
    if (created === 2)
      node.addEventListener = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      };
    return node;
  };
  assert.throws(
    () => new CompletionScrollbar(viewport as unknown as HTMLElement, () => {}),
    (error: unknown) => error === raw,
  );
  assert.ok(carrier);
  assert.equal((carrier as Node).removed, 1);
});

test('unchanged placement retains capture while an actual READ scale change retires it', () => {
  const f = fixture();
  f.flush();
  pointer(f.thumb, 'pointerdown');
  f.flush();
  f.flush();
  assert.equal(f.thumb.captures.has(7), true);
  f.viewport.physicalHeight = 300;
  f.flush();
  assert.equal(f.thumb.captures.has(7), false);
  f.scrollbar.destroy();
});
