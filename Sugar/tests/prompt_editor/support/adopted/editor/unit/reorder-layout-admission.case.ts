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
/** Actual bounded layout admission preserves node identities, occurrence budgets and native collection observations. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  admitReorderLineLayout,
  admitReorderTargetLayout,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Existing native DOM transport supplies actual mutable parent/child identities. */
type Paint = ReturnType<typeof motionPaintFixture>;
/** Narrow the established fixture's native element transport at the external DOM boundary. */
function element(node: ReturnType<Paint['document']['createElement']>): HTMLElement {
  return node as unknown as HTMLElement;
}
/** Compare only closed identity booleans; a failed assertion never dumps a painted subtree. */
function sameNodes(actual: readonly Node[] | null, expected: readonly object[]): void {
  assert.equal(actual !== null, true);
  assert.equal(actual!.length, expected.length);
  assert.equal(
    actual!.every((node, index) => node === expected[index]),
    true,
  );
}
/** Semantic target metadata carries no competing layout or source authority. */
function target(
  node: HTMLElement,
  index: number,
  kind: MountedReorderTarget['kind'] = 'chip',
): MountedReorderTarget {
  return { node, index, kind, leases: [], origins: [] };
}

test('line admission preserves root stack order and native child preorder with exact identities', () => {
  const f = motionPaintFixture(),
    first = f.document.createElement('span'),
    last = f.document.createElement('span'),
    left = f.document.createElement('span'),
    right = f.document.createElement('span'),
    nested = f.text('x');
  last.append(left, right);
  left.append(nested);
  sameNodes(admitReorderLineLayout([element(first), element(last)]), [
    last,
    left,
    nested,
    right,
    first,
  ]);
});

test('8192 actual nodes are admitted inclusively and a sibling pending frontier refuses before observation', () => {
  const f = motionPaintFixture(),
    root = f.document.createElement('span'),
    children = Array.from({ length: 8191 }, () => f.document.createElement('span'));
  root.append(...children);
  sameNodes(admitReorderLineLayout([element(root)]), [root, ...children]);
  const pendingSibling = f.document.createElement('span');
  let visitedChildren = 0;
  Object.defineProperty(children[0]!, 'childNodes', {
    get() {
      visitedChildren++;
      throw new Error('controlled-unreachable-child');
    },
  });
  assert.equal(admitReorderLineLayout([element(pendingSibling), element(root)]) === null, true);
  assert.equal(visitedChildren, 0);
  const extra = f.document.createElement('span');
  root.append(extra);
  assert.equal(admitReorderLineLayout([element(root)]) === null, true);
  assert.equal(visitedChildren, 0);
});

test('16384 observed UTF-16 characters are admitted inclusively across distinct text nodes', () => {
  const f = motionPaintFixture(),
    root = f.document.createElement('span'),
    left = f.text('x'.repeat(8192)),
    right = f.text('x'.repeat(8192));
  root.append(left, right);
  sameNodes(admitReorderLineLayout([element(root)]), [root, left, right]);
  root.append(f.text('x'));
  assert.equal(admitReorderLineLayout([element(root)]) === null, true);
});

test('semantic targets share the same complete node and character budget', () => {
  const f = motionPaintFixture(),
    first = f.document.createElement('span'),
    second = f.document.createElement('span');
  first.append(...Array.from({ length: 4095 }, () => f.document.createElement('span')));
  second.append(...Array.from({ length: 4095 }, () => f.document.createElement('span')));
  const targets = [target(element(first), 0), target(element(second), 1)];
  const admitted = admitReorderTargetLayout(targets);
  assert.equal(admitted?.size, 2);
  sameNodes(admitted!.get(element(first))!, [first, ...first.childNodes]);
  sameNodes(admitted!.get(element(second))!, [second, ...second.childNodes]);
  const overBudget = f.document.createElement('span');
  let childReads = 0;
  Object.defineProperty(overBudget, 'childNodes', {
    get() {
      childReads++;
      throw new Error('controlled-over-budget-child');
    },
  });
  assert.equal(
    admitReorderTargetLayout([...targets, target(element(overBudget), 2)]) === null,
    true,
  );
  assert.equal(childReads, 0);
  second.append(f.document.createElement('span'));
  assert.equal(admitReorderTargetLayout(targets) === null, true);
});

test('repeated text carriers count each actual semantic occurrence and preserve returned multiplicity', () => {
  const f = motionPaintFixture(),
    parent = f.document.createElement('span'),
    carrier = f.text('x'.repeat(8192)),
    first = f.document.createElement('span'),
    second = f.document.createElement('span');
  parent.append(carrier, first, second);
  const targets = [target(element(first), 0, 'gap'), target(element(second), 1, 'partition')];
  const admitted = admitReorderTargetLayout(targets)!;
  sameNodes(admitted.get(element(first))!, [carrier, first]);
  sameNodes(admitted.get(element(second))!, [carrier, second]);
  Object.defineProperty(carrier, 'length', { value: 8193 });
  assert.equal(admitReorderTargetLayout(targets) === null, true);
});

for (const length of [
  -1,
  0.5,
  Number.NaN,
  Number.POSITIVE_INFINITY,
  Number.MAX_SAFE_INTEGER + 1,
  16385,
]) {
  test("impossible observed text lengths refuse before reading that text's children", () => {
    const f = motionPaintFixture(),
      root = f.document.createElement('span'),
      text = f.text('');
    let childReads = 0;
    Object.defineProperty(text, 'length', { value: length });
    Object.defineProperty(text, 'childNodes', {
      get() {
        childReads++;
        throw new Error('controlled-invalid-text-child');
      },
    });
    root.append(text);
    assert.equal(admitReorderLineLayout([element(root)]) === null, true);
    assert.equal(childReads, 0);
  });
}

test('target admission returns the actual observed child collection with one collection and count read per node', () => {
  const f = motionPaintFixture(),
    root = f.document.createElement('span'),
    first = f.document.createElement('span'),
    second = f.document.createElement('span'),
    third = f.text('x');
  root.append(first, second, third);
  const observed = root.childNodes;
  const reads = [root, first, second, third].map((node) => {
    const stats = { collection: 0, count: 0 },
      children = node.childNodes,
      collection = new Proxy(children, {
        /** Observe native reflection without widening collection values. */
        get(value: typeof children, property: string | symbol, receiver: unknown): unknown {
          if (property === 'length') stats.count++;
          return Reflect.get(value, property, receiver);
        },
      });
    Object.defineProperty(node, 'childNodes', {
      get() {
        stats.collection++;
        return collection;
      },
    });
    return stats;
  });
  const admitted = admitReorderTargetLayout([target(element(root), 0)])!;
  sameNodes(admitted.get(element(root))!, [root, first, second, third]);
  assert.equal(observed[0] === first && observed[1] === second && observed[2] === third, true);
  assert.deepEqual(
    reads,
    Array.from({ length: 4 }, () => ({ collection: 1, count: 1 })),
  );
});
