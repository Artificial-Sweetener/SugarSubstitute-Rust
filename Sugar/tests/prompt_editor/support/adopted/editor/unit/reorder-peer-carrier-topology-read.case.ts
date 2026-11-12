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
/** Carrier validation reuses only the active full-shell native topology; all physical fragments and separate paint budgets stay live. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import {
  admitReorderLineLayout,
  admitReorderTargetLayout,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import { nativeReorderFootprint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-footprint.ts';
import { observeReorderSlotLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';

test('carrier validation reuses actual copied child/text topology after full-shell admission with identical complete output and live geometry operations', () =>
  usingAtomicPaintFixture((f) => {
    const source = atomicScenePaintSource(f, 0),
      owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        [source],
        1,
        1,
        () => true,
      );
    assert.ok(owner);
    const restore: (() => void)[] = [],
      restorationCauses: unknown[] = [];
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const carrier = f.f.line.querySelector('.sp-reorder-motion-peer')!,
        root = carrier.firstElementChild! as unknown as HTMLElement;
      const nodes = admitReorderLineLayout([root]);
      assert.ok(nodes);
      const operations: (readonly [string, Node])[] = [];
      for (const node of nodes) {
        if (node.nodeType !== 1) continue;
        const element = node as HTMLElement,
          rectangles = element.getClientRects.bind(element),
          bounds = element.getBoundingClientRect.bind(element);
        element.getClientRects = () => {
          operations.push(['rectangles', node]);
          return rectangles();
        };
        element.getBoundingClientRect = () => {
          operations.push(['bounds', node]);
          return bounds();
        };
      }
      const expected = nativeReorderFootprint(root, nodes, source.id);
      assert.ok(expected);
      assert.deepEqual(expected, source.rects);
      const expectedOperations = operations.slice();
      operations.length = 0;
      const children = nodes.map((node) => {
        const descriptor = Object.getOwnPropertyDescriptor(node, 'childNodes')!,
          original = node.childNodes;
        let reads = 0;
        Object.defineProperty(node, 'childNodes', {
          configurable: true,
          get() {
            if (++reads > 3) throw new Error('copied-topology-rediscovered-after-paint-admission');
            return original;
          },
        });
        restore.push(() => Object.defineProperty(node, 'childNodes', descriptor));
        return () => reads;
      });
      const lengths = nodes
        .filter((node) => node.nodeType === 3)
        .map((node) => {
          const descriptor = Object.getOwnPropertyDescriptor(node, 'length'),
            original = (node as Text).length;
          let reads = 0;
          Object.defineProperty(node, 'length', {
            configurable: true,
            get() {
              if (++reads > 2) throw new Error('copied-text-rediscovered-after-paint-admission');
              return original;
            },
          });
          restore.push(() => {
            if (descriptor) Object.defineProperty(node, 'length', descriptor);
            else if (!Reflect.deleteProperty(node, 'length'))
              throw new Error('controlled-length-restore-failed');
          });
          return () => reads;
        });
      const observers = f.f.nativeMutations.active;
      const result = observeReorderSlotLayout(
        f.f.line as unknown as HTMLElement,
        () => true,
        (read, authority) =>
          owner.read([source], () => authority.guard(authority.available), read.targets),
      );
      assert.equal(result, 'ready');
      assert.deepEqual(
        children.map((read) => read()),
        nodes.map(() => 3),
      );
      assert.deepEqual(
        lengths.map((read) => read()),
        lengths.map(() => 2),
      );
      assert.deepEqual(operations, expectedOperations);
      for (const release of restore)
        try {
          release();
        } catch (cause) {
          restorationCauses.push(cause);
        }
      restore.length = 0;
      assert.equal(owner.handles(source.id), true);
      assert.equal(f.f.nativeMutations.active, observers);
      assert.equal(root.textContent, f.roots[0]!.textContent);
      assert.equal(carrier.querySelectorAll('canvas').length, 1);
      assert.equal(f.draws.length, 1);
      assert.equal(f.draws[0]!.source, f.canvases[0]);
    } catch (cause) {
      f.causes.push(cause);
    }
    f.causes.push(...restorationCauses);
    for (const release of [...restore, () => owner.destroy()])
      try {
        release();
      } catch (cause) {
        f.causes.push(cause);
      }
  }));

for (const mutation of ['topology', 'geometry'] as const)
  test(`same-READ ${mutation} mutation cannot publish cached carrier validation or read later geometry`, () =>
    usingAtomicPaintFixture((f) => {
      const source = atomicScenePaintSource(f, 0),
        owner = ReorderMotionPeerCarriers.prepare(
          f.view,
          f.f.line as unknown as HTMLElement,
          [source],
          1,
          1,
          () => true,
        );
      assert.ok(owner);
      try {
        assert.equal(
          owner.instantiate(() => true),
          true,
        );
        const root = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
        const rectangles = root.getClientRects.bind(root),
          child = root.firstElementChild!;
        let changed = false,
          descendantReads = 0;
        const childRectangles = child.getClientRects.bind(child);
        child.getClientRects = () => {
          descendantReads++;
          return childRectangles();
        };
        if (mutation === 'geometry')
          root.getClientRects = () => {
            root.setAttribute('data-foreign', 'changed');
            changed = true;
            return rectangles();
          };
        const observers = f.f.nativeMutations.active;
        const result = observeReorderSlotLayout(
          f.f.line as unknown as HTMLElement,
          () => true,
          (read, authority) => {
            if (mutation === 'topology') {
              root.append(f.f.text('foreign'));
              changed = true;
            }
            return owner.read([source], () => authority.guard(authority.available), read.targets);
          },
        );
        assert.equal(result, null);
        assert.equal(changed, true);
        assert.equal(descendantReads, 0);
        assert.equal(owner.handles(source.id), false);
        assert.equal(f.f.nativeMutations.active, observers);
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        owner.destroy();
      } catch (cause) {
        f.causes.push(cause);
      }
    }));

for (const failure of [null, undefined, new Error('required-carrier-target-admission')])
  test(`required target admission keeps raw ${String(failure)} after native paint admission`, () =>
    usingAtomicPaintFixture((f) => {
      const source = atomicScenePaintSource(f, 0),
        owner = ReorderMotionPeerCarriers.prepare(
          f.view,
          f.f.line as unknown as HTMLElement,
          [source],
          1,
          1,
          () => true,
        );
      assert.ok(owner);
      try {
        assert.equal(
          owner.instantiate(() => true),
          true,
        );
        let caught: unknown,
          thrown = false,
          calls = 0;
        try {
          owner.read(
            [source],
            () => true,
            () => {
              calls++;
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw failure;
            },
          );
        } catch (cause) {
          thrown = true;
          caught = cause;
        }
        assert.equal(thrown, true);
        assert.equal(caught, failure);
        assert.equal(calls, 1);
        assert.equal(owner.handles(source.id), false);
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        owner.destroy();
      } catch (cause) {
        f.causes.push(cause);
      }
    }));

test('missing or retired copied-root membership never falls back to unobserved discovery', () =>
  usingAtomicPaintFixture((f) => {
    const source = atomicScenePaintSource(f, 0),
      owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        [source],
        1,
        1,
        () => true,
      );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const expired: { read: typeof admitReorderTargetLayout | null } = {
        read: null,
      };
      assert.equal(
        observeReorderSlotLayout(
          f.f.line as unknown as HTMLElement,
          () => true,
          (read) => {
            expired.read = read.targets;
            return true;
          },
        ),
        true,
      );
      assert.ok(expired.read);
      assert.equal(
        owner.read([source], () => true, expired.read),
        'discard',
      );
      assert.equal(
        owner.read(
          [source],
          () => true,
          () => new Map(),
        ),
        'discard',
      );
      assert.equal(owner.handles(source.id), false);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));

test('separate copied-media admission refuses before cached topology or any physical fragment read', () =>
  usingAtomicPaintFixture((f) => {
    const source = atomicScenePaintSource(f, 0),
      owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        [source],
        1,
        1,
        () => true,
      );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const root = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!,
        canvas = root.querySelector('canvas')!;
      Object.assign(canvas, { width: 1048577 });
      root.getClientRects = () => {
        throw new Error('inadmissible-media-read-geometry');
      };
      let calls = 0;
      assert.equal(
        owner.read(
          [source],
          () => true,
          () => {
            calls++;
            throw new Error('inadmissible-media-read-topology');
          },
        ),
        'discard',
      );
      assert.equal(calls, 0);
      assert.equal(owner.handles(source.id), false);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));

test('target-admission retirement cannot revive an owner or enter copied geometry even when the delegate returns valid actual topology', () =>
  usingAtomicPaintFixture((f) => {
    const source = atomicScenePaintSource(f, 0),
      owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        [source],
        1,
        1,
        () => true,
      );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const root = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
      let calls = 0,
        geometry = 0;
      root.getClientRects = () => {
        geometry++;
        throw new Error('retired-target-admission-read-geometry');
      };
      assert.equal(
        owner.read(
          [source],
          () => true,
          (targets) => {
            calls++;
            const actual = admitReorderTargetLayout(targets);
            assert.ok(actual);
            assert.equal(actual.has(root as unknown as HTMLElement), true);
            owner.destroy();
            return actual;
          },
        ),
        'unavailable',
      );
      assert.equal(calls, 1);
      assert.equal(geometry, 0);
      assert.equal(
        owner.read([source], () => true, admitReorderTargetLayout),
        'unavailable',
      );
      assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));
