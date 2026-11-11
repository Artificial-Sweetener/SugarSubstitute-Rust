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
/** Actual attachment output reuses coherent native style reflection while canonical fallback and owner fences remain live. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { attachReorderMotionStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { PaintNode } from './reorder-paint-node-fixture.ts';

/** Transport a retained native attribute independently of the fixture's attribute enumerator and its CSSOM accessor. */
function reflection(node: PaintNode) {
  const attribute = node.getAttribute.bind(node),
    descriptor = Object.getOwnPropertyDescriptor(node.style, 'cssText');
  assert.ok(descriptor?.get);
  const read = descriptor.get;
  const state = { value: attribute('style'), reads: 0, fail: false };
  node.getAttribute = (name) => (name === 'style' ? state.value : attribute(name));
  Object.defineProperty(node.style, 'cssText', {
    ...descriptor,
    get: () => {
      ++state.reads;
      if (state.fail) throw new Error('warm-canonical-read-forbidden');
      return Reflect.apply(read, node.style, []) as string;
    },
  });
  return {
    state,
    /** Retire each independent override even when another retirement fails. */
    restore(): void {
      const causes: unknown[] = [];
      for (const restore of [
        () => {
          node.getAttribute = attribute;
        },
        () => Object.defineProperty(node.style, 'cssText', descriptor),
      ])
        try {
          restore();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'reflection-retirement-failed');
    },
  };
}

/** One genuine clip lease starts from the independent literal viewport and actual production declarations. */
function fixture() {
  const f = motionPaintFixture(),
    node = f.document.createElement('span');
  node.className = 'sp-reorder-motion-stage';
  node.style.width = '200px';
  node.style.height = '256px';
  const lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {});
  assert.ok(lease);
  f.drawLayer();
  const mountNode = f.layerRoot.childNodes[0];
  assert.ok(mountNode instanceof PaintNode);
  return { f, node, lease, mountNode };
}

test('retained reflected output avoids both CSSOM serializations without changing complete viewport output', () => {
  const { f, node, lease, mountNode } = fixture(),
    mount = reflection(mountNode),
    root = reflection(f.layerRoot);
  const causes: unknown[] = [];
  try {
    const before = [
      f.layerMount.style.cssText,
      f.layerRoot.style.cssText,
      [mountNode.box.left, mountNode.box.top, mountNode.box.width, mountNode.box.height],
      [f.layerRoot.box.left, f.layerRoot.box.top, f.layerRoot.box.width, f.layerRoot.box.height],
      f.scroller.scrollWidth,
      f.scroller.scrollHeight,
    ];
    mount.state.reads = root.state.reads = 0;
    mount.state.fail = root.state.fail = true;
    assert.equal(lease.current(), true);
    assert.equal(lease.read(), 'ready');
    assert.equal(mount.state.reads, 0);
    assert.equal(root.state.reads, 0);
    assert.deepEqual(
      [
        mount.state.value,
        root.state.value,
        [mountNode.box.left, mountNode.box.top, mountNode.box.width, mountNode.box.height],
        [f.layerRoot.box.left, f.layerRoot.box.top, f.layerRoot.box.width, f.layerRoot.box.height],
        f.scroller.scrollWidth,
        f.scroller.scrollHeight,
      ],
      before,
    );
    mount.state.fail = root.state.fail = false;
    mount.state.value += ' ';
    assert.equal(lease.current(), true);
    assert.equal(mount.state.reads, 1);
    assert.equal(root.state.reads, 0);
    mount.state.value = null;
    assert.equal(lease.current(), true);
    assert.equal(mount.state.reads, 2);
    f.layerMount.style.setProperty('transform', 'scale(2)', 'important');
    mount.state.value = 'transform: scale(2) !important;';
    assert.equal(lease.current(), false);
    assert.equal(mount.state.reads, 3);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => mount.restore(),
    () => root.restore(),
    () => lease.release(),
    () => node.remove(),
    () => f.destroyLayer(),
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'reflection-proof-failed');
});

for (const cause of [null, undefined, Object.freeze({})]) {
  test('warm native reflection failures preserve their exact raw cause', () => {
    const { f, node, lease } = fixture(),
      attribute = f.layerMount.getAttribute.bind(f.layerMount);
    const causes: unknown[] = [];
    try {
      f.layerMount.getAttribute = (name) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'style') throw cause;
        return attribute(name);
      };
      assert.throws(
        () => lease.current(),
        (actual: unknown) => {
          assert.equal(actual, cause);
          return true;
        },
      );
    } catch (primary) {
      causes.push(primary);
    }
    for (const release of [
      () => {
        f.layerMount.getAttribute = attribute;
      },
      () => lease.release(),
      () => node.remove(),
      () => f.destroyLayer(),
    ])
      try {
        release();
      } catch (cleanup) {
        causes.push(cleanup);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reflection-error-proof-failed');
  });
}

test('stamp refuses an old canonical declaration paired with foreign new reflected spelling', () => {
  const { f, node, lease } = fixture(),
    attribute = f.layerMount.getAttribute.bind(f.layerMount),
    descriptor = Object.getOwnPropertyDescriptor(f.layerMount.style, 'cssText');
  assert.ok(descriptor?.get);
  const read = descriptor.get;
  let spelling = attribute('style'),
    changed = false;
  const causes: unknown[] = [];
  try {
    f.layerMount.getAttribute = (name) => (name === 'style' ? spelling : attribute(name));
    Object.defineProperty(f.layerMount.style, 'cssText', {
      ...descriptor,
      get: () => {
        const old = Reflect.apply(read, f.layerMount.style, []) as string;
        if (!changed) {
          changed = true;
          spelling = 'width: 99px;';
          f.layerMount.style.setProperty('width', '99px');
        }
        return old;
      },
    });
    f.attachment.write(f.layerMount, null);
    assert.equal(changed, true);
    assert.equal(f.attachment.current(), false);
    assert.equal(lease.read(), 'unavailable');
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => {
      f.layerMount.getAttribute = attribute;
    },
    () => Object.defineProperty(f.layerMount.style, 'cssText', descriptor),
    () => node.remove(),
    () => f.destroyLayer(),
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'reflection-coherence-proof-failed');
});

for (const frontier of ['stamp', 'fallback'] as const) {
  for (const raw of [null, undefined]) {
    test(`canonical ${frontier} preserves raw native getter failures`, () => {
      const { f, node, lease, mountNode } = fixture(),
        transported = reflection(mountNode),
        descriptor = Object.getOwnPropertyDescriptor(mountNode.style, 'cssText');
      assert.ok(descriptor?.get);
      const read = descriptor.get;
      let failed = false;
      const causes: unknown[] = [];
      try {
        if (frontier === 'fallback') transported.state.value += ' ';
        Object.defineProperty(mountNode.style, 'cssText', {
          ...descriptor,
          get: () => {
            if (!failed) {
              failed = true;
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw raw;
            }
            return Reflect.apply(read, mountNode.style, []) as string;
          },
        });
        assert.throws(
          () => (frontier === 'stamp' ? f.attachment.write(f.layerMount, null) : lease.current()),
          (actual: unknown) => {
            assert.equal(actual, raw);
            return true;
          },
        );
        assert.equal(failed, true);
      } catch (cause) {
        causes.push(cause);
      }
      for (const release of [
        () => transported.restore(),
        () => lease.release(),
        () => node.remove(),
        () => f.destroyLayer(),
      ])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'canonical-error-proof-failed');
    });
  }
}

for (const owner of ['mount', 'root'] as const) {
  test(`warm ${owner} reflection cannot retain an owner retired inside its native getter`, () => {
    const { f, node, lease, mountNode } = fixture(),
      subject = owner === 'mount' ? mountNode : f.layerRoot,
      attribute = subject.getAttribute.bind(subject);
    let retired = false;
    const causes: unknown[] = [];
    try {
      subject.getAttribute = (name) => {
        const before = attribute(name);
        if (name === 'style' && !retired) {
          retired = true;
          f.attachment.destroy();
        }
        return before;
      };
      assert.equal(lease.current(), false);
      assert.equal(retired, true);
      assert.equal(lease.read(), 'unavailable');
      assert.equal(f.layerRoot.isConnected, false);
    } catch (cause) {
      causes.push(cause);
    }
    for (const release of [
      () => {
        subject.getAttribute = attribute;
      },
      () => lease.release(),
      () => node.remove(),
      () => f.destroyLayer(),
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reflection-reentry-proof-failed');
  });
}

test('two absent reflected attributes cannot bypass the canonical declaration', () => {
  const { f, node, lease, mountNode } = fixture(),
    transported = reflection(mountNode);
  const causes: unknown[] = [];
  try {
    transported.state.value = null;
    f.attachment.write(f.layerMount, null);
    transported.state.reads = 0;
    assert.equal(lease.current(), true);
    assert.equal(transported.state.reads, 1);
    mountNode.style.setProperty('width', '99px');
    assert.equal(lease.current(), false);
    assert.equal(transported.state.reads, 2);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => transported.restore(),
    () => lease.release(),
    () => node.remove(),
    () => f.destroyLayer(),
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'absent-reflection-proof-failed');
});
