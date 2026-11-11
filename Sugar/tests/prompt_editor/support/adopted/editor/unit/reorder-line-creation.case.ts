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
/** The actual line owner retains every nested canonical factory result under one current key bucket. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ReorderLineLayout,
  type ReorderLineLease,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';

test('same-key nested creation retains both actual nodes, reborrows without creation and retires each once', () => {
  const owner = new ReorderLineLayout(2);
  const retired = [0, 0],
    removed = [0, 0];
  const nodes = [0, 1].map(
    (id) =>
      ({
        remove() {
          removed[id]!++;
        },
      }) as unknown as HTMLElement,
  );
  const leases: ReorderLineLease[] = [];
  try {
    const outer = owner.borrow('same-origin', () => {
      const inner = owner.borrow('same-origin', () => ({
        node: nodes[1]!,
        destroy() {
          retired[1]!++;
        },
      }));
      leases.push(inner);
      inner.measure('literal-context', { height: 7.5, rects: [] });
      return {
        node: nodes[0]!,
        destroy() {
          retired[0]!++;
        },
      };
    });
    assert.equal(outer.node, nodes[0]);
    assert.equal(leases[0]!.node, nodes[1]);
    owner.reconcile(() => {
      outer.release();
      leases[0]!.release();
    });
    const first = owner.borrow('same-origin', () => assert.fail('nested-node-lost'));
    const second = owner.borrow('same-origin', () => assert.fail('nested-node-lost'));
    assert.equal(new Set([first.node, second.node]).size, 2);
    assert.ok(nodes.every((node) => node === first.node || node === second.node));
    assert.deepEqual(first.geometry('literal-context'), {
      height: 7.5,
      rects: [],
    });
    assert.deepEqual(second.geometry('literal-context'), {
      height: 7.5,
      rects: [],
    });
    assert.deepEqual(retired, [0, 0]);
    first.release();
    second.release();
    owner.destroy();
    owner.destroy();
    assert.deepEqual(retired, [1, 1]);
    assert.ok(removed.every((count) => count > 0));
  } finally {
    owner.destroy();
  }
});
