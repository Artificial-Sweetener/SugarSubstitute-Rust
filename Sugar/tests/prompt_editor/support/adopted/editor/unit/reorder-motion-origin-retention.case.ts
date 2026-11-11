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
/** Actual presentation retains complete observed origins while revealing only the immutable changed-target plan. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderState } from '../transaction/reorder-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

for (const capturedBeforePublication of [true, false])
  test(`actual origins captured ${capturedBeforePublication ? 'before' : 'after'} the next Core issue preserve selected samples and unmasked native peers`, () => {
    const f = motionPresentationFixture();
    let next = false;
    const create = f.paint.document.createElement.bind(f.paint.document);
    /** Apply the independent next-order geometry to original retained nodes and future allocations; Core supplies identity/order only. */
    const place = (node: ReturnType<typeof create>) => {
      const nativeRect = node.getBoundingClientRect.bind(node);
      node.getBoundingClientRect = () => {
        const value = nativeRect();
        let target: typeof node | null = node;
        while (target && target.dataset.reorderChip === undefined) target = target.parent;
        const id = target?.dataset.reorderChip;
        if (!next || (id !== '0' && id !== '2')) return value;
        const left = id === '0' ? 100 : 60;
        return {
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          ...value,
          left,
          right: left + value.width,
          x: left,
        } as DOMRect;
      };
      return node;
    };
    f.paint.document.createElement = (name) => place(create(name));
    for (const node of f.paint.line.querySelectorAll()) place(node);
    try {
      const selection = f.state.selection;
      f.prepare();
      f.deliver();
      assert.deepEqual(f.state.field(reorderPreview).mask?.ids, [1, 0]);
      f.paint.step(90);
      const origin = f.presentation.before();
      assert.equal(new Set(origin.map((rect) => rect.index)).size, 3);
      assert.equal(origin.find((rect) => rect.index === 1)?.left, 30);
      assert.equal(origin.find((rect) => rect.index === 0)?.left, 55);
      assert.equal(origin.find((rect) => rect.index === 2)?.left, 100);
      const nextOrder = moveReorderChip(f.input.capture.prepared, f.input.order, 0, {
        kind: 'row',
        row: 0,
        insertion: 2,
      });
      assert.ok(nextOrder);
      f.input.order = nextOrder;
      f.publish();
      next = true;
      const preview = f.state.field(reorderPreview).value;
      assert.ok(preview);
      assert.equal(
        new ReorderWidget(preview, f.state.field(reorderPreview).mask).updateDOM(f.root, f.view),
        true,
      );
      const mixed = f.presentation.before();
      assert.equal(mixed.find((rect) => rect.index === 1)?.left, 30);
      assert.equal(mixed.find((rect) => rect.index === 0)?.left, 55);
      assert.equal(mixed.find((rect) => rect.index === 2)?.left, 60);
      f.presentation.propose(f.input, capturedBeforePublication ? origin : mixed);
      f.deliver();
      assert.deepEqual(
        f.state.field(reorderPreview).mask?.ids,
        capturedBeforePublication ? [1, 2, 0] : [1, 0],
      );
      const issued = f.presentation.before();
      assert.equal(new Set(issued.map((rect) => rect.index)).size, 3);
      assert.equal(
        issued.find((rect) => rect.index === 2)?.left,
        capturedBeforePublication ? 100 : 60,
      );
      f.paint.step(270);
      const destination = f.presentation.before();
      assert.equal(destination.find((rect) => rect.index === 2)?.left, 60);
      assert.equal(destination.find((rect) => rect.index === 0)?.left, 100);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('empty changed selection accepts all complete actual geometry without revealing or masking native peers', () => {
  const f = motionPresentationFixture();
  try {
    const actual = [
      {
        kind: 'chip' as const,
        index: 1,
        left: 20,
        right: 40,
        top: 20,
        bottom: 40,
      },
      {
        kind: 'chip' as const,
        index: 0,
        left: 60,
        right: 80,
        top: 20,
        bottom: 40,
      },
      {
        kind: 'chip' as const,
        index: 2,
        left: 100,
        right: 120,
        top: 20,
        bottom: 40,
      },
    ];
    f.presentation.propose(f.input, actual);
    f.deliver();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(f.view.dom.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.deepEqual(
      f.presentation.before(),
      actual.flatMap((rect) => [rect, rect]),
    );
    f.presentation.propose(f.input, actual);
    assert.equal(f.jobs.length, 0);
  } finally {
    f.destroy();
  }
});

test('65-plus actually changed refusal keeps same-issue dedupe while the next origin reads all actual native groups', () => {
  const f = motionPresentationFixture(
    reorderState(Array.from({ length: 66 }, (_, id) => String(id)).join(', ')).state,
  );
  try {
    Object.assign(f.paint.scroller.box, { width: 4000 });
    const source = readExactSource(f.state),
      selection = f.state.selection;
    f.presentation.propose(
      f.input,
      f.origin.map((rect) => ({
        ...rect,
        left: rect.left + 10,
        right: rect.right + 10,
      })),
    );
    f.deliver();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    const origin = f.presentation.before();
    assert.equal(new Set(origin.map((rect) => rect.index)).size, 66);
    assert.equal(origin.find((rect) => rect.index === 65)?.left, 2620);
    f.presentation.propose(f.input, origin);
    assert.equal(f.jobs.length, 0);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('mixed current native and retained selected complete groups retain the4096 aggregate without partial truncation', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.deliver();
    f.paint.step(90);
    f.publish();
    const preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    assert.equal(
      new ReorderWidget(preview, f.state.field(reorderPreview).mask).updateDOM(f.root, f.view),
      true,
    );
    const roots = Array.from(f.root.querySelectorAll<HTMLElement>('[data-reorder-chip]'));
    assert.equal(roots.length, 3);
    let pieces = 4091;
    for (const root of roots) {
      const box = root.getBoundingClientRect();
      root.getClientRects = () => {
        const rects =
          root.dataset.reorderChip === '2'
            ? Array.from({ length: pieces }, () => box)
            : // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
              [{ ...box, left: -10, right: -5 }];
        return rects as unknown as DOMRectList;
      };
    }
    assert.equal(f.presentation.before().length, 4096);
    pieces = 4092;
    assert.deepEqual(f.presentation.before(), []);
    assert.ok(f.state.field(reorderPreview).mask);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('empty-plan retirement reentry cannot borrow a new accepted geometry cache for an older retained scene', () => {
  const f = motionPresentationFixture(),
    create = f.paint.document.createElement.bind(f.paint.document);
  let issued: unknown = null,
    armed = false;
  let sampled: ReturnType<typeof f.presentation.before> | null = null;
  f.paint.document.createElement = (name) => {
    const node = create(name);
    const set = node.style.setProperty.bind(node.style);
    let writes = 0;
    node.style.setProperty = (name, value, priority) => {
      set(name, value, priority);
      if (name !== 'clip') return;
      writes++;
      if (!armed || writes < 2 || node === issued || node.className !== 'sp-reorder-motion-paint')
        return;
      armed = false;
      const peer = Array.from(f.root.querySelectorAll<HTMLElement>('[data-reorder-chip]')).find(
        (root) => root.dataset.reorderChip === '2',
      );
      assert.ok(peer);
      for (const piece of [peer, ...peer.querySelectorAll<HTMLElement>('*')]) {
        const box = piece.getBoundingClientRect();
        piece.getClientRects = () =>
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          [{ ...box, left: 110, right: 110 + box.width }] as unknown as DOMRectList;
      }
      sampled = f.presentation.before();
    };
    return node;
  };
  try {
    const source = readExactSource(f.state),
      selection = f.state.selection;
    f.prepare();
    f.deliver();
    f.paint.step(90);
    issued = f.view.dom.querySelector('.sp-reorder-motion-paint');
    assert.ok(issued);
    f.publish();
    const preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    assert.equal(
      new ReorderWidget(preview, f.state.field(reorderPreview).mask).updateDOM(f.root, f.view),
      true,
    );
    const actual = [
      {
        kind: 'chip' as const,
        index: 1,
        left: 20,
        right: 40,
        top: 20,
        bottom: 40,
      },
      {
        kind: 'chip' as const,
        index: 0,
        left: 60,
        right: 80,
        top: 20,
        bottom: 40,
      },
      {
        kind: 'chip' as const,
        index: 2,
        left: 100,
        right: 120,
        top: 20,
        bottom: 40,
      },
    ];
    armed = true;
    f.presentation.propose(f.input, actual);
    f.deliver();
    assert.ok(sampled);
    const observed: ReturnType<typeof f.presentation.before> = sampled;
    assert.equal(observed.find((rect) => rect.index === 2)?.left, 110);
    assert.equal(observed.find((rect) => rect.index === 1)?.left, 30);
    assert.equal(observed.find((rect) => rect.index === 0)?.left, 55);
    assert.equal(f.tasks.length, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.presentation.hit(35, 30), null);
    assert.equal(f.state.field(reorderPreview).mask === null, true);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(redoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
