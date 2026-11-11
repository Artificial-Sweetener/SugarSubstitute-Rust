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
/** Native mutation transports prove the finite observation boundary independently of Scene/mask publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionObservation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-observation.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';

/** Actual StateField publication and literal native subscriptions remain independent of the observation policy. */
function fixture() {
  let transport: ReturnType<typeof visibilityObserverFixture> | null = null;
  const f = motionPresentationFixture(undefined, (document) => {
    transport = visibilityObserverFixture(document);
  });
  assert.ok(transport);
  let changed = 0;
  const observation = new ReorderMotionObservation(
    f.view,
    f.state.field(reorderPreview).value!.capture,
    {
      owned: () => true,
      current: () => true,
      observe: (action) => action(),
      changed: () => changed++,
    },
  );
  return {
    f,
    observation,
    transport: transport as ReturnType<typeof visibilityObserverFixture>,
    get changed() {
      return changed;
    },
  };
}

test('pending native records preserve the exact root metadata cap and reject content mutations before root drainage', () => {
  const { f, observation, transport } = fixture();
  try {
    observation.start();
    assert.equal(transport.active, 2);
    for (let at = 0; at < 64; at++)
      f.view.dom.setAttribute('data-reorder-motion-issue', String(at));
    assert.equal(
      observation.cleanPending(() => true),
      true,
    );
    for (let at = 0; at < 65; at++)
      f.view.dom.setAttribute('data-reorder-motion-issue', String(at));
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    f.root.setAttribute('data-foreign', '1');
    f.view.dom.setAttribute('data-foreign-root', '1');
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    // Content rejection leaves the genuine root record pending for its own later adjudication.
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    assert.equal(
      observation.cleanPending(() => true),
      true,
    );
  } finally {
    try {
      assert.deepEqual(observation.disconnect(), []);
      assert.equal(transport.active, 0);
    } finally {
      f.destroy();
    }
  }
});

for (const target of ['root', 'content'] as const)
  test(`retirement before native ${target} observe delegation cannot reconnect the captured subscription or affect a newer acquisition`, () => {
    const fixtureState = fixture(),
      { f, observation, transport } = fixtureState,
      window = f.view.dom.ownerDocument.defaultView!,
      NativeObserver = window.MutationObserver,
      callbacks: {
        readonly callback: MutationCallback;
        readonly observer: MutationObserver;
      }[] = [],
      primary = {};
    let retired = false;
    /** The native call genuinely delegates after reentrant retirement, then preserves its original raw failure. */
    class Observer extends NativeObserver {
      constructor(callback: MutationCallback) {
        super(callback);
        callbacks.push({ callback, observer: this });
      }
      override observe(node: Node, options?: MutationObserverInit): void {
        if (!retired && node === (target === 'root' ? f.view.dom : f.view.contentDOM)) {
          retired = true;
          assert.deepEqual(observation.disconnect(), []);
          super.observe(node, options);
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        }
        super.observe(node, options);
      }
    }
    Object.defineProperty(window, 'MutationObserver', {
      configurable: true,
      value: Observer,
    });
    let newer: ReorderMotionObservation | null = null;
    try {
      let failure: { readonly cause: unknown } | null = null;
      try {
        observation.start();
      } catch (cause) {
        failure = { cause };
      }
      assert.ok(failure);
      assert.equal(failure.cause, primary);
      assert.equal(retired, true);
      assert.equal(transport.active, 0);
      const oldCallbacks = [...callbacks];
      newer = new ReorderMotionObservation(f.view, f.state.field(reorderPreview).value!.capture, {
        owned: () => true,
        current: () => true,
        observe: (action) => action(),
        changed: () => assert.fail('unexpected-new-observation-invalidation'),
      });
      newer.start();
      assert.equal(transport.active, 2);
      for (const { callback, observer } of oldCallbacks) callback([], observer);
      assert.equal(fixtureState.changed, 0);
      assert.deepEqual(observation.disconnect(), []);
      assert.equal(transport.active, 2);
    } finally {
      newer?.disconnect();
      observation.disconnect();
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value: NativeObserver,
      });
      try {
        assert.equal(transport.active, 0);
      } finally {
        f.destroy();
      }
    }
  });

test('content and root disconnect failures settle both native targets and return raw causes in resource cleanup order', () => {
  const { f, observation, transport } = fixture(),
    window = f.view.dom.ownerDocument.defaultView!,
    NativeObserver = window.MutationObserver,
    order: string[] = [];
  let acquired = 0;
  /** Actual native disconnect still runs before each independently failing boundary returns its raw cause. */
  class Observer extends NativeObserver {
    private readonly role = ++acquired === 1 ? 'content' : 'root';
    override disconnect(): void {
      order.push(this.role);
      super.disconnect();
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw this.role === 'content' ? null : undefined;
    }
  }
  Object.defineProperty(window, 'MutationObserver', {
    configurable: true,
    value: Observer,
  });
  try {
    observation.start();
    assert.equal(transport.active, 2);
    const failures = observation.disconnect();
    assert.equal(failures.length, 2);
    assert.equal(failures[0], null);
    assert.equal(failures[1], undefined);
    assert.deepEqual(order, ['content', 'root']);
    assert.equal(transport.active, 0);
    assert.deepEqual(observation.disconnect(), []);
    assert.deepEqual(order, ['content', 'root']);
  } finally {
    observation.disconnect();
    Object.defineProperty(window, 'MutationObserver', {
      configurable: true,
      value: NativeObserver,
    });
    f.destroy();
  }
});
