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
/** Actual canonical block reconciliation minimizes native sibling writes while retaining rich resources and source authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  ReorderWidget,
  type PassiveReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

/** An independent quadratic LCS oracle supplies the minimum insertions/moves from actual sibling identities. */
function minimumSiblingWrites(before: readonly ChildNode[], after: readonly ChildNode[]): number {
  let previous = new Array<number>(after.length + 1).fill(0);
  for (const old of before) {
    const next = new Array<number>(after.length + 1).fill(0);
    for (let index = 0; index < after.length; ++index)
      next[index + 1] =
        old === after[index] ? previous[index]! + 1 : Math.max(previous[index + 1]!, next[index]!);
    previous = next;
  }
  return after.length - previous[after.length]!;
}

/** The existing prepared raster/CM fixture supplies real canonical owners; only native insertion is observed. */
function blockFixture() {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    painted: HTMLCanvasElement[] = [];
  let f: ReturnType<typeof motionPresentationFixture> | null = null,
    paint: PassiveReorderPaint | null = null;
  /** Independent teardown also owns partially constructed fixtures and restores the external document boundary. */
  const destroy = (): void => {
    const causes: unknown[] = [];
    for (const release of [
      () => paint?.destroy(),
      () => f?.destroy(),
      () => {
        if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'minimal-block-proof-retirement');
  };
  try {
    const fixture = motionPresentationFixture(
      preparedLoraState((canvas) => painted.push(canvas), 4),
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
    );
    f = fixture;
    const capture = fixture.input.capture,
      token = fixture.input.token,
      initialOrder = capture.prepared.initial;
    /** Every proposed order is issued by Core and the original preview preparation owner. */
    const preview = (order: typeof initialOrder) => {
      const value = prepareReorderPreview({ capture, token, activeChip: 0, order }, 'complete');
      assert.ok(value);
      return value;
    };
    const initial = preview(initialOrder),
      owner = new ReorderWidget(initial, null).mountPassive(fixture.view);
    paint = owner;
    fixture.line.append(owner.node as unknown as (typeof fixture.chips)[number]);
    const source = readExactSource(fixture.state),
      selection = fixture.state.selection,
      insert = owner.node.insertBefore.bind(owner.node);
    let writes = 0,
      afterInsert: (() => void) | null = null;
    owner.node.insertBefore = (node, reference) => {
      ++writes;
      const result = insert(node, reference);
      afterInsert?.();
      return result;
    };
    return {
      fixture,
      owner,
      initial,
      initialOrder,
      preview,
      painted,
      originalText: owner.node.textContent,
      /** Native write counts reset independently of the production plan and sibling policy. */
      reset(): void {
        writes = 0;
      },
      get writes() {
        return writes;
      },
      /** Synchronous native callbacks exercise the existing generation/retirement fences. */
      onInsert(callback: () => void): void {
        afterInsert = callback;
      },
      /** Actual CM state owns source, directional selection and undo/redo throughout readonly proposals. */
      assertSource(): void {
        assert.equal(readExactSource(fixture.state) === source, true);
        assert.equal(fixture.state.selection === selection, true);
        assert.equal(undoDepth(fixture.state), 0);
        assert.equal(redoDepth(fixture.state), 0);
      },
      /** Genuine Core movement preserves independently authored endpoint intent. */
      move(order: typeof initialOrder, chip: number, insertion: number) {
        const next = moveReorderChip(capture.prepared, order, chip, {
          kind: 'row',
          row: 0,
          insertion,
        });
        assert.ok(next);
        return { order: next, preview: preview(next) };
      },
      destroy,
    };
  } catch (primary) {
    try {
      destroy();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'minimal-block-fixture-acquisition');
    }
    throw primary;
  }
}

test('bidirectional repeated rich chips retain every canonical child and canvas with the minimum actual sibling writes', () => {
  const f = blockFixture();
  try {
    const original = f.owner
        .targets()
        .filter((target) => target.kind === 'chip')
        .map((target) => ({
          id: target.index,
          node: target.node,
          child: target.node.childNodes[0],
        })),
      canvases = [...f.owner.node.querySelectorAll<HTMLCanvasElement>('.sp-lora-media')],
      paintCount = f.painted.length;
    assert.equal(original.length, 4);
    assert.equal(canvases.length, 4);
    let order = f.initialOrder;
    for (const insertion of [3, 0, 2, 1, 3, 0]) {
      const before = [...f.owner.node.childNodes],
        next = f.move(order, 0, insertion);
      order = next.order;
      f.reset();
      assert.equal(f.owner.reconcile(next.preview), 'ready');
      const after = [...f.owner.node.childNodes];
      assert.equal(f.writes, minimumSiblingWrites(before, after));
      assert.equal(new Set(after).size, after.length);
      assert.equal(f.owner.node.textContent === f.originalText, true);
      const targets = f.owner.targets().filter((target) => target.kind === 'chip');
      assert.deepEqual(
        targets.map((target) => target.index),
        order.order,
      );
      assert.deepEqual(
        [...f.owner.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')].map((node) =>
          Number(node.dataset.reorderChip),
        ),
        order.order,
      );
      for (const old of original) {
        const current = targets.find((target) => target.index === old.id);
        assert.ok(current);
        assert.equal(current.node === old.node, true);
        assert.equal(current.node.childNodes[0] === old.child, true);
      }
      const currentCanvases = [...f.owner.node.querySelectorAll('.sp-lora-media')];
      assert.equal(currentCanvases.length, canvases.length);
      assert.equal(
        canvases.every((canvas) => currentCanvases.includes(canvas)),
        true,
      );
      assert.equal(f.painted.length, paintCount);
      f.assertSource();
      f.reset();
      assert.equal(f.owner.reconcile(next.preview), 'ready');
      assert.equal(f.writes, 0);
    }
  } finally {
    f.destroy();
  }
});

test('invalidated canonical inner paint is replaced once while surviving siblings use minimum writes', () => {
  const f = blockFixture();
  try {
    const targets = f.owner.targets().filter((target) => target.kind === 'chip'),
      broken = targets[0]!,
      surviving = targets.slice(1).map((target) => target.node),
      before = [...f.owner.node.childNodes];
    assert.ok(broken.node.childNodes[0]);
    broken.node.childNodes[0].remove();
    f.reset();
    assert.equal(new ReorderWidget(f.initial, null).updateDOM(f.owner.node, f.fixture.view), true);
    const after = [...f.owner.node.childNodes],
      current = f.owner.targets().filter((target) => target.kind === 'chip');
    assert.equal(f.writes, minimumSiblingWrites(before, after));
    assert.equal(current[0]!.node !== broken.node, true);
    assert.equal(
      surviving.every((node) => current.some((target) => target.node === node)),
      true,
    );
    assert.equal(f.owner.node.textContent === f.originalText, true);
    f.assertSource();
  } finally {
    f.destroy();
  }
});

for (const action of ['replace', 'retire'] as const)
  test(`native insertion ${action} reentry stops old writes and preserves the current owner`, () => {
    const f = blockFixture();
    try {
      const next = f.move(f.initialOrder, 0, 3);
      let entered = false,
        newer = false,
        writesAtRetirement = -1;
      f.onInsert(() => {
        if (entered) return;
        entered = true;
        if (action === 'replace')
          newer = new ReorderWidget(f.initial, null).updateDOM(f.owner.node, f.fixture.view);
        else f.owner.destroy();
        writesAtRetirement = f.writes;
      });
      f.reset();
      assert.equal(f.owner.reconcile(next.preview), 'retired');
      assert.equal(entered, true);
      assert.equal(f.writes, writesAtRetirement);
      if (action === 'replace') {
        assert.equal(newer, true);
        assert.equal(f.owner.current(), true);
        assert.equal(f.owner.node.textContent === f.originalText, true);
        assert.deepEqual(
          [...f.owner.node.querySelectorAll<HTMLElement>('[data-reorder-chip]')].map((node) =>
            Number(node.dataset.reorderChip),
          ),
          f.initialOrder.order,
        );
      } else assert.equal(f.owner.current(), false);
      f.assertSource();
    } finally {
      f.destroy();
    }
  });

for (const raw of [null, undefined])
  test(`native insertion raw ${String(raw)} failure retires incomplete canonical output`, () => {
    const f = blockFixture();
    try {
      const next = f.move(f.initialOrder, 0, 3);
      f.owner.node.insertBefore = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      };
      let caught = false;
      try {
        f.owner.reconcile(next.preview);
      } catch (cause) {
        caught = true;
        assert.equal(cause === raw, true);
      }
      assert.equal(caught, true);
      assert.equal(f.owner.current(), false);
      f.assertSource();
    } finally {
      f.destroy();
    }
  });
