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
/** Actual source recipes keep inert mapped presentation while semantic command capabilities retire. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView } from '@codemirror/view';
import { structureState } from '../transaction/document-structure-fixture.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { nativeReorderDisplay } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-display.ts';
import { nativeReorderRecipeDecorations } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-recipe.ts';
import {
  currentNativeReorder,
  prepareNativeReorder,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import {
  reorderPreview,
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** The existing native preparation and state owners issue every specimen and capability. */
function prepared() {
  const fixture = structureState('a, b', 'prompt', [authorshipExtension, reorderPreviewExtension]);
  return { ...fixture, state: initializeNativeStructure(fixture.state) };
}

/** Ordered marks expose complete literal endpoints while retaining actual Decoration identities independently of counts. */
function marks(decorations: DecorationSet) {
  const result: {
    readonly from: number;
    readonly to: number;
    readonly id: string | null;
    readonly value: Decoration;
  }[] = [];
  for (const cursor = decorations.iter(); cursor.value; cursor.next()) {
    if (cursor.from === cursor.to) continue;
    result.push({
      from: cursor.from,
      to: cursor.to,
      id: cursor.value.spec.attributes?.['data-reorder-chip'] ?? null,
      value: cursor.value,
    });
  }
  return result;
}

test('pending source preparation maps actual inert marks without preserving capture authority', () => {
  const before = prepared().state,
    display = before.field(nativeReorderDisplay),
    original = marks(display),
    capture = captureReorder(before),
    preview = prepareReorderPreview(
      {
        token: {},
        capture,
        activeChip: capture.active,
        order: capture.prepared.initial,
      },
      'complete',
    );
  assert.ok(preview);
  const entered = before.update({
    effects: installReorderPreview.of(preview),
  }).state;
  const pending = entered.update({
    changes: { from: 4, insert: ', tail' },
  }).state;
  assert.equal(pending.field(semanticStructures).native, null);
  assert.equal(pending.field(reorderPreview).value, null);
  assert.equal(currentNativeReorder(pending, capture), false);
  assert.equal(prepareNativeReorder(pending, capture.context, { from: 0, to: 0 }), null);
  const mapped = pending.field(nativeReorderDisplay),
    observed = marks(mapped);
  assert.deepEqual(
    observed.map(({ from, to, id }) => [from, to, id]),
    [
      [0, 1, '0'],
      [1, 3, null],
      [3, 10, '1'],
    ],
  );
  assert.equal(observed[0]!.value, original[0]!.value);
  assert.equal(observed[1]!.value, original[1]!.value);
  assert.equal(observed[2]!.value, original[2]!.value);
  assert.ok(pending.facet(EditorView.outerDecorations).includes(mapped));
  const rejected = pending.update({
    effects: installReorderPreview.of(preview),
  }).state;
  assert.equal(rejected.field(reorderPreview).value, null);
  assert.equal(rejected.field(nativeReorderDisplay), mapped);
  const fresh = initializeNativeStructure(rejected),
    values = fresh.field(semanticStructures).native!.nativeValues;
  assert.equal(fresh.field(nativeReorderDisplay), nativeReorderRecipeDecorations(values));
  assert.deepEqual(
    marks(fresh.field(nativeReorderDisplay)).map(({ from, to, id }) => [from, to, id]),
    [
      [0, 1, '0'],
      [1, 3, null],
      [3, 4, '1'],
      [4, 6, null],
      [6, 10, '2'],
    ],
  );
  assert.equal(readExactSource(fresh), 'a, b, tail');
  assert.equal(currentNativeReorder(fresh, capture), false);
  assert.ok(captureReorder(fresh));
});

test('mapped display uses logical UTF16 while readonly, kind, clear and fresh-editor lifetimes retain separate authority', () => {
  const fixture = prepared(),
    before = fixture.state,
    original = marks(before.field(nativeReorderDisplay)),
    capture = captureReorder(before);
  const pending = before.update({ changes: { from: 0, insert: '🙂\n' } }).state;
  const mapped = pending.field(nativeReorderDisplay),
    observed = marks(mapped);
  assert.deepEqual(
    observed.map(({ from, to, id }) => [from, to, id]),
    [
      [0, 4, '0'],
      [4, 6, null],
      [6, 7, '1'],
    ],
  );
  assert.equal(observed[0]!.value, original[0]!.value);
  assert.equal(readExactSource(pending), '🙂\na, b');
  const readonly = pending.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(readonly.field(nativeReorderDisplay), mapped);
  assert.equal(currentNativeReorder(readonly, capture), false);
  assert.equal(prepareNativeReorder(readonly, capture.context, { from: 3, to: 3 }), null);
  const changedKind = pending.update({
    effects: fixture.kindSlot.reconfigure(documentKind.of('wildcard-txt')),
  }).state;
  assert.equal(changedKind.field(semanticStructures).native, null);
  assert.equal(changedKind.field(nativeReorderDisplay), Decoration.none);
  const cleared = pending.update({
    changes: { from: 0, to: pending.doc.length },
  }).state;
  assert.equal(cleared.field(nativeReorderDisplay), Decoration.none);
  assert.equal(cleared.field(semanticStructures).native, null);
  assert.equal(readExactSource(cleared), '');
  const reset = structureState('next', 'prompt', [
    authorshipExtension,
    reorderPreviewExtension,
  ]).state;
  assert.equal(reset.field(nativeReorderDisplay), Decoration.none);
  assert.equal(reset.field(reorderPreview).value, null);
  assert.equal(reset.field(semanticStructures).native, null);
});
