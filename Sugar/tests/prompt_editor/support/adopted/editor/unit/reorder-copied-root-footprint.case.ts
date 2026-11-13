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
/** Copied roots reduce only private numeric operands; every rich descendant retains guarded native observation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  nativeReorderFootprint,
  NativeReorderRootSample,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-footprint.ts';
import { observeReorderSlotLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import type { ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Literal viewport geometry is independent of either reduction path. */
function box(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    right: left + width,
    top,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({ left, top, width, height }),
  };
}
/** Only the external iterable native-list boundary is transported. */
function list(rects: DOMRect[]): DOMRectList {
  return Object.assign(rects, {
    item: (index: number) => rects[index] ?? null,
  });
}
/** One actual literal native transport owns subscriptions and preserves every body/cleanup cause. */
function complete(action: (f: ReturnType<typeof motionPaintFixture>) => void): void {
  const f = motionPaintFixture(),
    causes: unknown[] = [];
  try {
    action(f);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => assert.equal(f.nativeMutations.active, 0), () => f.destroyLayer()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'copied-root-proof-retirement');
}

const expected: readonly ReorderRect[] = [
  { kind: 'chip', index: 7, left: 1, right: 13, top: 2, bottom: 18 },
  { kind: 'chip', index: 7, left: 2, right: 5, top: 4, bottom: 8 },
];

test('private copied-root reduction preserves complete actual output and native reads while avoiding repeated observer drains', () => {
  complete((f) => {
    const document = f.view.dom.ownerDocument,
      window = document.defaultView!,
      descriptor = Object.getOwnPropertyDescriptor(window, 'MutationObserver')!,
      Native = window.MutationObserver,
      causes: unknown[] = [];
    let drains = 0;
    class Observer extends Native {
      /** Count actual native queue drains without replacing mutation delivery or owner authority. */
      override takeRecords(): MutationRecord[] {
        drains++;
        return super.takeRecords();
      }
    }
    const results: {
      copied: boolean;
      footprintDrains: number;
      rootReads: number;
      descendantReads: number;
      rects: readonly ReorderRect[];
    }[] = [];
    try {
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value: Observer,
      });
      for (const copied of [false, true]) {
        const shell = document.createElement('span'),
          root = document.createElement('span'),
          descendant = document.createElement('span');
        root.append(descendant);
        shell.append(root);
        let rootReads = 0,
          descendantReads = 0,
          footprintDrains = -1;
        root.getClientRects = () => {
          rootReads++;
          return list([box(1, 2, 12, 16)]);
        };
        descendant.getClientRects = () => {
          descendantReads++;
          return list([box(2, 4, 3, 4)]);
        };
        const rects = observeReorderSlotLayout(
          shell,
          () => true,
          (topology, authority) => {
            /** Native checkpoints borrow the actual finite Slot mutation fence. */
            const current = (): boolean => authority.guard(authority.available);
            const nodes = topology.targets([
              { node: root, index: 7, kind: 'chip', leases: [], origins: [] },
            ]);
            assert.ok(nodes);
            let sample: NativeReorderRootSample | null = null;
            if (copied) {
              assert.equal(current(), true);
              const native = root.getClientRects();
              assert.equal(current(), true);
              sample = NativeReorderRootSample.capture(native, current, 4096);
              assert.ok(sample);
              assert.equal(Object.isFrozen(sample.fragments), true);
              assert.equal(Object.isFrozen(sample.fragments[0]), true);
            }
            const start = drains;
            const output = nativeReorderFootprint(
              root,
              nodes.get(root)!,
              7,
              undefined,
              sample ? { sample, current } : { current },
            );
            footprintDrains = drains - start;
            return output;
          },
        );
        assert.ok(rects);
        assert.deepEqual(rects, expected);
        assert.equal(rootReads, 1);
        assert.equal(descendantReads, 1);
        assert.equal(Object.isFrozen(rects), true);
        results.push({
          copied,
          footprintDrains,
          rootReads,
          descendantReads,
          rects,
        });
      }
      assert.deepEqual(
        results.map(({ footprintDrains }) => footprintDrains),
        [34, 26],
      );
      assert.equal(results[0]!.footprintDrains - results[1]!.footprintDrains, 8);
      assert.deepEqual(results[0]!.rects, results[1]!.rects);
      assert.deepEqual(
        results.map(({ rootReads, descendantReads }) => [rootReads, descendantReads]),
        [
          [1, 1],
          [1, 1],
        ],
      );
    } catch (cause) {
      causes.push(cause);
    }
    try {
      Object.defineProperty(window, 'MutationObserver', descriptor);
    } catch (cause) {
      causes.push(cause);
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'copied-root-drain-proof-retirement');
  });
});

test('nominal root extraction rejects a stale request before touching a hostile native list', () => {
  let reads = 0;
  const hostile = new Proxy(list([]), {
    get() {
      reads++;
      throw new Error('stale-root-list-consumed');
    },
  });
  assert.equal(
    NativeReorderRootSample.capture(hostile, () => false, 4096),
    null,
  );
  assert.equal(reads, 0);
});

test('root count refusal precedes all native fragment access', () => {
  let reads = 0;
  const hostile = new Proxy(list([]), {
    get(_target, key) {
      if (key === 'length') return 4097;
      reads++;
      throw new Error('overbudget-root-item-consumed');
    },
  });
  assert.equal(
    NativeReorderRootSample.capture(hostile, () => true, 4096),
    null,
  );
  assert.equal(reads, 0);
});

for (const boundary of ['index', 'field'] as const)
  test(`copied native ${boundary} mutation refuses before later rich native reads`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        root = document.createElement('span');
      shell.append(root);
      let later = 0;
      const rect = box(1, 2, 12, 16);
      const native =
        boundary === 'index'
          ? new Proxy(list([rect]), {
              get(target, key, receiver) {
                if (key === '0') shell.setAttribute('data-proof', 'changed');
                return Reflect.get(target, key, receiver);
              },
            })
          : list([
              new Proxy(rect, {
                get(target, key, receiver) {
                  if (key === 'left') shell.setAttribute('data-proof', 'changed');
                  return Reflect.get(target, key, receiver);
                },
              }),
            ]);
      const output = observeReorderSlotLayout(
        shell,
        () => true,
        (_topology, authority) => {
          /** Native checkpoints borrow the actual finite Slot mutation fence. */
          const current = (): boolean => authority.guard(authority.available);
          const sample = NativeReorderRootSample.capture(native, current, 4096);
          if (!sample) return null;
          later++;
          return sample;
        },
      );
      assert.equal(output, null);
      assert.equal(later, 0);
    });
  });

for (const raw of [null, undefined, Object.freeze({ code: 'copied-root-native-failure' })])
  test('native fragment field failures retain their exact raw cause', () => {
    const native = list([
      new Proxy(box(1, 2, 12, 16), {
        get() {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        },
      }),
    ]);
    let caught = false,
      cause: unknown;
    try {
      NativeReorderRootSample.capture(native, () => true, 4096);
    } catch (error) {
      caught = true;
      cause = error;
    }
    assert.equal(caught, true);
    assert.equal(cause === raw, true);
  });

for (const boundary of ['return', 'length', 'field'] as const)
  test(`copied root leaves live descendant ${boundary} mutation and hostile-return fences intact`, () => {
    complete((f) => {
      const document = f.view.dom.ownerDocument,
        shell = document.createElement('span'),
        root = document.createElement('span'),
        descendant = document.createElement('span');
      root.append(descendant);
      shell.append(root);
      let consumed = 0;
      const rect = box(2, 4, 3, 4);
      descendant.getClientRects = () => {
        if (boundary === 'return') {
          shell.setAttribute('data-proof', 'changed');
          return new Proxy(list([]), {
            get() {
              consumed++;
              throw new Error('stale-descendant-list-consumed');
            },
          });
        }
        if (boundary === 'length')
          return new Proxy(list([rect]), {
            get(target, key, receiver) {
              if (key === 'length') shell.setAttribute('data-proof', 'changed');
              if (key === Symbol.iterator || key === '0') {
                consumed++;
                throw new Error('stale-descendant-iteration-consumed');
              }
              return Reflect.get(target, key, receiver);
            },
          });
        return list([
          new Proxy(rect, {
            get(target, key, receiver) {
              if (key === 'left') shell.setAttribute('data-proof', 'changed');
              return Reflect.get(target, key, receiver);
            },
          }),
        ]);
      };
      const output = observeReorderSlotLayout(
        shell,
        () => true,
        (topology, authority) => {
          /** Native checkpoints borrow the actual finite Slot mutation fence. */
          const current = (): boolean => authority.guard(authority.available);
          const sample = NativeReorderRootSample.capture(list([box(1, 2, 12, 16)]), current, 4096);
          assert.ok(sample);
          const nodes = topology.targets([
            { node: root, index: 7, kind: 'chip', leases: [], origins: [] },
          ]);
          assert.ok(nodes);
          return nativeReorderFootprint(root, nodes.get(root)!, 7, undefined, {
            sample,
            current,
          });
        },
      );
      assert.equal(output, null);
      assert.equal(consumed, 0);
    });
  });

test('nominal reduction rejects a structurally similar foreign object instead of accepting its numeric array', () => {
  /** This exact invalid boundary operand proves runtime private provenance rather than TypeScript assignability alone. */
  const foreign: unknown = { fragments: [box(1, 2, 12, 16)] };
  assert.throws(
    () => NativeReorderRootSample.append(foreign as NativeReorderRootSample, [], 7),
    TypeError,
  );
});

test('native rectangle mutation after extraction cannot change the actual immutable root output', () => {
  const native = box(1, 2, 12, 16),
    sample = NativeReorderRootSample.capture(list([native]), () => true, 4096);
  assert.ok(sample);
  const fragments = sample.fragments;
  Object.defineProperty(native, 'left', { value: 900 });
  const rects: ReorderRect[] = [];
  assert.equal(NativeReorderRootSample.append(sample, rects, 7), true);
  assert.deepEqual(rects, [expected[0]]);
  assert.equal(sample.fragments === fragments, true);
});

for (const invalid of ['empty', 'coordinate'] as const)
  test(`copied numeric reduction preserves ${invalid} filtering and admission`, () => {
    const sample = NativeReorderRootSample.capture(
      list([box(invalid === 'coordinate' ? NaN : 1, 2, invalid === 'empty' ? 0 : 12, 16)]),
      () => true,
      4096,
    );
    assert.ok(sample);
    const rects: ReorderRect[] = [];
    const result = NativeReorderRootSample.append(sample, rects, 7);
    assert.equal(result, invalid === 'empty');
    assert.deepEqual(rects, []);
  });

test('copied roots retain the exact inclusive whole-footprint occurrence budget', () => {
  const sample = NativeReorderRootSample.capture(
    list(Array.from({ length: 4096 }, () => box(1, 2, 12, 16))),
    () => true,
    4096,
  );
  assert.ok(sample);
  const accepted: ReorderRect[] = [],
    refused: ReorderRect[] = [expected[0]!];
  assert.equal(NativeReorderRootSample.append(sample, accepted, 7), true);
  assert.equal(accepted.length, 4096);
  for (const rect of accepted) assert.deepEqual(rect, expected[0]);
  assert.equal(NativeReorderRootSample.append(sample, refused, 7), false);
  assert.deepEqual(refused, [expected[0]]);
});

test('reflective construction cannot install an arbitrary numeric array as an issued root sample', () => {
  let consumed = 0;
  const foreign = new Proxy([], {
    get() {
      consumed++;
      throw new Error('foreign-root-array-consumed');
    },
  });
  assert.throws(() => Reflect.construct(NativeReorderRootSample, [Symbol('foreign'), foreign]), {
    message: 'reorder-root-sample-authority-required',
  });
  assert.equal(consumed, 0);
});

test('viewport-qualified copied roots retain getter mutation fences before later native descendants', () => {
  complete((f) => {
    const document = f.view.dom.ownerDocument,
      shell = document.createElement('span'),
      root = document.createElement('span'),
      descendant = document.createElement('span');
    root.append(descendant);
    shell.append(root);
    let reads = 0;
    descendant.getClientRects = () => {
      reads++;
      throw new Error('stale-viewport-descendant-read');
    };
    const output = observeReorderSlotLayout(
      shell,
      () => true,
      (topology, authority) => {
        /** Native checkpoints borrow the actual finite Slot mutation fence. */
        const current = (): boolean => authority.guard(authority.available);
        const sample = NativeReorderRootSample.capture(
          list([box(1, 2, 12, 16), box(1, 18, 12, 16)]),
          current,
          4096,
        );
        assert.ok(sample);
        const nodes = topology.targets([
          { node: root, index: 7, kind: 'chip', leases: [], origins: [] },
        ]);
        assert.ok(nodes);
        return nativeReorderFootprint(
          root,
          nodes.get(root)!,
          7,
          {
            get left() {
              shell.setAttribute('data-proof', 'changed');
              return 0;
            },
            right: 20,
            top: 0,
            bottom: 40,
          },
          { sample, current },
        );
      },
    );
    assert.equal(output, null);
    assert.equal(reads, 0);
  });
});
