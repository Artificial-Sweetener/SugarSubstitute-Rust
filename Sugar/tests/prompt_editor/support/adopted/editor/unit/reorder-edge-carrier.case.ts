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
/** Current publication geometry proves punctuated-edge marker placement, accepted navigation and native history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { publicationFixture, hideReorderPublication } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  reorderHeldMarker,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';

for (const [ordinal, specimen] of [
  { source: 'a\nb,', position: 2, expected: 'b, a\n,' },
  { source: 'a\nb,\n', position: 2, expected: 'b, a\n,\n' },
  { source: 'a\nb,  ', position: 2, expected: 'b, a\n,  ' },
  { source: 'a\nb,\n\n', position: 2, expected: 'b, a\n,\n\n' },
  { source: 'a\n\nb,', position: 3, expected: 'a\nb\n,' },
  { source: 'a\nb', position: 2, expected: 'b, a\n' },
  { source: 'a,\nb', position: 3, expected: 'b, a\n' },
].entries())
  for (const early of [false, true])
    test(`punctuated singleton Up uses current origin and commits once, specimen=${ordinal}, early=${early}`, () => {
      const transport = publicationFixture(specimen.source, specimen.position),
        { f } = transport;
      try {
        const selection = f.state.selection;
        f.owner.keydown(key('Alt'));
        transport.settle();
        assert.equal(f.owner.keydown(key('ArrowUp')), true);
        if (early) f.owner.keyup(key('Alt'));
        transport.settle();
        if (!early) {
          assert.equal(readExactSource(f.state), specimen.source);
          assert.equal(f.state.selection, selection);
          assert.equal(undoDepth(f.state), 0);
          f.owner.keyup(key('Alt'));
          transport.settle();
        }
        assert.equal(readExactSource(f.state), specimen.expected);
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(undo(f.view), true);
        assert.equal(readExactSource(f.state), specimen.source);
        assert.equal(redo(f.view), true);
        assert.equal(readExactSource(f.state), specimen.expected);
        assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
      } finally {
        f.destroy();
      }
    });

test('current hidden punctuated carrier exposes the genuine marker on the held physical line', () => {
  const transport = publicationFixture('a\nb,  \n', 2),
    { f } = transport;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    const full = f.state.field(reorderPreview).value!;
    f.owner.keydown(key('ArrowUp'));
    const hidden = hideReorderPublication(transport);
    assert.deepEqual(hidden.order.order, [0]);
    const rects = Array.from(
      transport.collisionTargets(),
      (node): ReorderRect => ({
        kind: node.dataset.reorderChip !== undefined ? 'chip' : 'gap',
        index: Number(node.dataset.reorderChip ?? node.dataset.reorderGap),
        ...(node.dataset.reorderBlank !== undefined
          ? { blank: Number(node.dataset.reorderBlank) }
          : {}),
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...node.getClientRects()[0]!,
      }),
    );
    assert.deepEqual(
      reorderHeldMarker(full.order, hidden.order, 1, rects, {
        left: 0,
        right: 200,
        top: 0,
        bottom: 120,
      }),
      { left: 10, right: 12, top: 40, bottom: 60 },
    );
    assert.equal(readExactSource(f.state), 'a\nb,  \n');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('repeated punctuated Up/Down/Up consumes current publications and one final history event', () => {
  const source = 'a\nb,',
    transport = publicationFixture(source, 2),
    { f } = transport;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    for (const arrow of ['ArrowUp', 'ArrowDown', 'ArrowUp']) {
      f.owner.keydown(key(arrow));
      transport.settle();
    }
    assert.equal(readExactSource(f.state), source);
    assert.equal(undoDepth(f.state), 0);
    f.owner.keyup(key('Alt'));
    transport.settle();
    assert.equal(readExactSource(f.state), 'b, a\n,');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

for (const source of ['a\nb,', 'a,\nb'])
  test(`trailing boundary restores exact source and selection without a history event, source length=${source.length}`, () => {
    const transport = publicationFixture(source, source.indexOf('b')),
      { f } = transport;
    try {
      const selection = f.state.selection;
      f.owner.keydown(key('Alt'));
      transport.settle();
      f.owner.keydown(key('ArrowDown'));
      transport.settle();
      f.owner.keyup(key('Alt'));
      transport.settle();
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });

for (const retire of ['source', 'selection', 'dispose'] as const)
  test(`punctuated hidden read retires without publication after ${retire}`, () => {
    const transport = publicationFixture('a\nb,', 2),
      { f } = transport;
    try {
      f.owner.keydown(key('Alt'));
      transport.settle();
      f.owner.keydown(key('ArrowUp'));
      hideReorderPublication(transport);
      const pending = f.jobs.shift()!;
      assert.ok(pending);
      const result = pending.read(f.view);
      if (retire === 'source')
        f.view.dispatch({
          changes: { from: 0, to: 1, insert: 'z' },
          annotations: Transaction.addToHistory.of(false),
        });
      else if (retire === 'selection') f.view.dispatch({ selection: EditorSelection.cursor(0) });
      else f.owner.destroy();
      const after = readExactSource(f.state);
      pending.write?.(result, f.view);
      transport.settle();
      assert.equal(readExactSource(f.state), after);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });
