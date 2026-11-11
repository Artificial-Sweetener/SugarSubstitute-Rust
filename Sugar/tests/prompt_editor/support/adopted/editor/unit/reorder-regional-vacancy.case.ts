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
/** Actual CM session publication proves regional vacancy navigation without a geometry fallback. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, StateEffect, Transaction } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { publicationFixture, hideReorderPublication } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  reorderHeldMarker,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';

for (const early of [false, true])
  test(`regional singleton Up uses its genuine vacancy and commits one exact-source history entry, early=${early}`, () => {
    const source = 'a\nb,\n[SEP|Face]\nc',
      transport = publicationFixture(source, 2),
      { f } = transport;
    try {
      assert.equal(f.owner.keydown(key('Alt')), true);
      transport.settle();
      const selection = f.state.selection;
      assert.equal(f.owner.keydown(key('ArrowUp')), true);
      if (early) f.owner.keyup(key('Alt'));
      transport.settle();
      if (!early) {
        assert.equal(readExactSource(f.state), source);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(f.state.selection, selection);
        assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 0, 2]);
        f.owner.keyup(key('Alt'));
        transport.settle();
      }
      const expected = 'b, a\n\n[SEP|Face]\nc';
      assert.equal(readExactSource(f.state), expected);
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state), source);
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state), expected);
      assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
    } finally {
      f.destroy();
    }
  });

test('cancelling a regional preview restores authored punctuation and history', () => {
  const source = 'a\nb,\n[SEP|Face]\nc',
    transport = publicationFixture(source, 2),
    { f } = transport;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    for (let step = 0; step < 3; step++) {
      f.owner.keydown(key('ArrowUp'));
      transport.settle();
    }
    f.owner.keydown(key('Escape'));
    transport.settle();
    assert.equal(readExactSource(f.state), source);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});

test('current regional hidden publication supplies its actual held marker without a full-layout fallback', () => {
  const source = 'a\nb,\n[SEP|Face]\nc',
    transport = publicationFixture(source, 2),
    { f } = transport;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    const full = f.state.field(reorderPreview).value!;
    f.owner.keydown(key('ArrowUp'));
    const hidden = hideReorderPublication(transport);
    assert.deepEqual(hidden.order.order, [0, 2]);
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
        bottom: 150,
      }),
      { left: 10, right: 12, top: 40, bottom: 60 },
    );
    assert.equal(readExactSource(f.state), source);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('repeated regional Up/Down/Up consumes current publications and commits once', () => {
  const source = 'a\nb,\n[SEP|Face]\nc',
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
    assert.equal(readExactSource(f.state), 'b, a\n\n[SEP|Face]\nc');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state), source);
  } finally {
    f.destroy();
  }
});

for (const retire of ['source', 'selection', 'profile', 'dispose'] as const)
  test(`regional hidden read cannot publish after ${retire} supersession`, () => {
    const source = 'a\nb,\n[SEP|Face]\nc',
      transport = publicationFixture(source, 2),
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
      else if (retire === 'profile')
        f.view.dispatch({
          effects: StateEffect.appendConfig.of(
            featureProfile.of(
              resolveFeatureProfile(
                {
                  ...defaultPreferences,
                  allowed: {
                    ...defaultPreferences.allowed,
                    segment_reorder: false,
                  },
                },
                { mode: 'library' },
              ),
            ),
          ),
        });
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

for (const ending of ['\n', '\r\n'])
  test(`regional non-BMP move retains exact newline spelling through native transactions, ending=${ending.length}`, () => {
    const source = `🦊${ending}b,${ending}[SEP|Face]${ending}c`,
      transport = publicationFixture(source, 3),
      { f } = transport;
    try {
      f.view.dispatch({
        ...exactReplacement(f.state, 0, readExactSource(f.state).length, source),
        selection: EditorSelection.cursor(3),
        annotations: Transaction.addToHistory.of(false),
      });
      f.view.dispatch({
        effects: installSemanticStructure.of({
          purpose: 'native',
          prepared: prepareNativeStructure(f.state),
        }),
        annotations: Transaction.addToHistory.of(false),
      });
      assert.equal(readExactSource(f.state), source);
      f.owner.keydown(key('Alt'));
      transport.settle();
      f.owner.keydown(key('ArrowUp'));
      transport.settle();
      f.owner.keyup(key('Alt'));
      transport.settle();
      assert.equal(readExactSource(f.state), `b, 🦊${ending}${ending}[SEP|Face]${ending}c`);
      assert.equal(undoDepth(f.state), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state), source);
    } finally {
      f.destroy();
    }
  });

test('regional Alt entry and release without a destination change preserves source, selection and history', () => {
  const source = 'a\nb,\n[SEP|Face]\nc',
    transport = publicationFixture(source, 2),
    { f } = transport;
  try {
    const selection = f.state.selection;
    f.owner.keydown(key('Alt'));
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
