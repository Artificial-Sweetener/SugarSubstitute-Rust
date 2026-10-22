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
/** Genuine deferred keyboard outcomes retain actual collision DOM, font and native context leases until source-neutral application. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { EditorSelection } from '@codemirror/state';
import { arrowGeometryFixture, calibrateCollision, drain } from './reorder-marker-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Stop on the actual ready read/write's queued outcome or the final native-context verification's queued apply. */
function queued(frontier: 'ready' | 'verified') {
  const transport = arrowGeometryFixture(),
    { f } = transport,
    fonts = new EventTarget();
  Object.defineProperty(f.view.dom.ownerDocument, 'fonts', { value: fonts });
  f.owner.keydown(key('Alt'));
  drain(f);
  const complete = f.state.field(reorderPreview).value!,
    selection = f.state.selection;
  f.owner.keydown(key('ArrowDown'));
  f.measure();
  f.task();
  calibrateCollision(f);
  const pending = f.jobs.shift()!;
  const measured = pending.read(f.view);
  assert.ok(measured && typeof measured === 'object');
  assert.equal(Reflect.get(measured, 'kind'), 'target');
  pending.write!(measured, f.view);
  assert.equal(f.tasks.length, 1);
  if (frontier === 'verified') {
    f.task();
    const verification = f.jobs.shift()!;
    const fresh = verification.read(f.view);
    assert.equal(fresh, true);
    verification.write!(fresh, f.view);
    assert.equal(f.tasks.length, 1);
  }
  const root = Array.from(f.view.dom.ownerDocument.body.childNodes).find((node) =>
    (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
  ) as HTMLElement;
  assert.ok(root?.isConnected);
  const paint = root.childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1] as HTMLElement;
  assert.ok(paint.isConnected);
  return { ...transport, fonts, root, paint, complete, selection };
}

for (const frontier of ['ready', 'verified'] as const)
  for (const invalidation of ['root', 'paint', 'font', 'context'] as const)
    for (const fifo of [false, true])
      test(`queued ${frontier} keyboard target renews actual ${invalidation} lease before application, FIFO=${fifo}`, () => {
        const t = queued(frontier),
          { f } = t;
        try {
          if (fifo) {
            f.owner.keydown(key('ArrowLeft'));
            f.owner.keyup(key('Alt'));
          }
          t.reflow();
          if (invalidation === 'root') t.root.remove();
          if (invalidation === 'paint') t.paint.remove();
          if (invalidation === 'font') t.fonts.dispatchEvent(new Event('loadingdone'));
          if (invalidation === 'context')
            (f.view.domAtPos(0).node as HTMLElement).className += ' changed-native-context';
          f.task();
          assert.equal(f.state.field(reorderPreview).value, t.complete);
          assert.equal(f.state.field(reorderPreview).value!.presentation, 'complete');
          assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [0, 1, 2]);
          assert.equal(f.state.selection, t.selection);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(undoDepth(f.state), 0);
          assert.ok(f.jobs.length);
          drain(f);
          if (fifo) {
            assert.equal(readExactSource(f.state), 'b, a, c');
            assert.equal(undoDepth(f.state), 1);
            assert.equal(f.state.field(reorderPreview).value, null);
          } else {
            assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 2, 0]);
            assert.equal(readExactSource(f.state), 'a, b, c');
            assert.equal(f.state.selection, t.selection);
            assert.equal(undoDepth(f.state), 0);
            f.owner.keydown(key('Escape'));
            drain(f);
          }
          assert.equal(t.root.isConnected, false);
          assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
        } finally {
          f.destroy();
        }
      });

for (const retirement of ['source', 'selection', 'dispose'] as const)
  test(`final verified queued geometry cannot publish after ${retirement} retirement`, () => {
    const t = queued('verified'),
      { f } = t;
    try {
      if (retirement === 'source') f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      if (retirement === 'selection') f.view.dispatch({ selection: EditorSelection.cursor(1) });
      if (retirement === 'dispose') f.owner.destroy();
      const state = f.state;
      assert.equal(f.owner.blocksInput(), false);
      f.task();
      drain(f);
      assert.equal(f.state.doc, state.doc);
      assert.equal(f.state.selection, state.selection);
      assert.equal(readExactSource(f.state), readExactSource(state));
      assert.equal(undoDepth(f.state), undoDepth(state));
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(t.root.isConnected, false);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });

test('final native verification observes changed computed width with unchanged literal DOM stamps before applying', () => {
  const t = queued('ready'),
    { f } = t;
  try {
    const line = f.view.domAtPos(0).node as HTMLElement,
      window = f.view.dom.ownerDocument.defaultView!,
      computed = window.getComputedStyle.bind(window),
      box = line.getBoundingClientRect.bind(line);
    let width = 100;
    window.getComputedStyle = ((node: Element) => {
      const style = computed(node);
      return node === line
        ? {
            // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
            ...style,
            width: `${width}px`,
            getPropertyValue: (name: string) =>
              name === 'width' ? `${width}px` : style.getPropertyValue(name),
          }
        : style;
    }) as typeof window.getComputedStyle;
    line.getBoundingClientRect = () => {
      const rect = box();
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      return { ...rect, right: rect.left + width, width } as DOMRect;
    };
    f.task();
    width = 160.375;
    t.reflow();
    const verification = f.jobs.shift()!;
    const fresh = verification.read(f.view);
    assert.equal(fresh, false);
    verification.write!(fresh, f.view);
    f.task();
    assert.equal(f.state.field(reorderPreview).value, t.complete);
    assert.equal(f.state.selection, t.selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    drain(f);
    assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 2, 0]);
    assert.equal(t.root.isConnected, false);
    f.owner.keydown(key('Escape'));
    drain(f);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

test('valid final geometry lease applies once with the exact warm collision root and paint retained', () => {
  const t = queued('verified'),
    { f } = t;
  try {
    f.task();
    assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 0, 2]);
    assert.equal(t.root.isConnected, true);
    assert.equal(t.paint.isConnected, true);
    assert.equal(t.root.childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1], t.paint);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, t.selection);
    assert.equal(undoDepth(f.state), 0);
    drain(f);
    f.owner.keyup(key('Alt'));
    drain(f);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(t.root.isConnected, false);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

test('final queued lease observation preserves primary then cleanup causes while retiring every finite resource', () => {
  const t = queued('verified'),
    { f } = t,
    primary = new Error('queued-context-observation-failed'),
    cleanup = new Error('queued-collision-retirement-failed'),
    remove = t.root.remove.bind(t.root);
  try {
    (f.view.domAtPos(0).node as HTMLElement).getAttribute = () => {
      throw primary;
    };
    t.root.remove = () => {
      remove();
      throw cleanup;
    };
    assert.throws(
      () => f.task(),
      (cause) =>
        cause instanceof AggregateError &&
        cause.errors[0] === primary &&
        cause.errors[1] === cleanup,
    );
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, t.selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(t.root.isConnected, false);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});
