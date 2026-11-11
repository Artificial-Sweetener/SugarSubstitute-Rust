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
/** Source-owned initial native ranges remain the actual shared command inputs through transient gesture lifetimes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorView } from '@codemirror/view';
import { ChangeSet, RangeSet, EditorState } from '@codemirror/state';
import { structureState } from '../transaction/document-structure-fixture.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  nativeReorderRecipe,
  nativeReorderRecipeDecorations,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-recipe.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Keep source, permission and all real native state owners in one explicit fixture. */
function stateFor(source: string, kind: 'prompt' | 'wildcard-txt' | 'wildcard-csv' = 'prompt') {
  const fixture = structureState(source, kind, [authorshipExtension, reorderPreviewExtension]);
  return { ...fixture, state: initializeNativeStructure(fixture.state) };
}
/** Initial output retains no gesture or editable authority, and is reused rather than merely recounted. */
test('native initial source recipe is actual Core command output and survives Alt entry and cancellation', () => {
  const fixture = stateFor('a, b, c'),
    before = fixture.state,
    values = before.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, values.entries[0]!)!,
    aggregate = nativeReorderRecipeDecorations(values),
    capture = captureReorder(before, 3);
  assert.ok(recipe);
  assert.equal(capture.prepared, recipe.prepared);
  assert.equal(capture.prepared.initial, recipe.prepared.initial);
  const expected: {
    from: number;
    to: number;
    id: string | null;
    className: string;
  }[] = [];
  // RangeSet.iter guarantees source order; between deliberately does not promise callback ordering.
  const cursor = recipe.decorations.iter();
  for (; cursor.value; cursor.next()) {
    const value = cursor.value;
    if (cursor.from === cursor.to) continue;
    expected.push({
      from: cursor.from,
      to: cursor.to,
      id: value.spec.attributes?.['data-reorder-chip'] ?? null,
      className: value.spec.class,
    });
  }
  assert.deepEqual(expected, [
    { from: 0, to: 1, id: '0', className: 'sp-reorder-chip' },
    { from: 1, to: 3, id: null, className: 'sp-reorder-gap' },
    { from: 3, to: 4, id: '1', className: 'sp-reorder-chip' },
    { from: 4, to: 6, id: null, className: 'sp-reorder-gap' },
    { from: 6, to: 7, id: '2', className: 'sp-reorder-chip' },
  ]);
  assert.ok(before.facet(EditorView.outerDecorations).includes(aggregate));
  const preview = prepareReorderPreview(
    { capture, order: capture.prepared.initial, activeChip: 1, token: {} },
    'complete',
  );
  assert.ok(preview);
  assert.equal(preview.output, recipe.output);
  const entered = before.update({
    effects: installReorderPreview.of(preview),
  }).state;
  assert.equal(entered.field(reorderPreview).native, true);
  assert.ok(entered.facet(EditorView.outerDecorations).includes(aggregate));
  const cancelled = entered.update({
    effects: installReorderPreview.of(null),
  }).state;
  assert.ok(cancelled.facet(EditorView.outerDecorations).includes(aggregate));
  assert.equal(nativeReorderRecipe(values, values.entries[0]!), recipe);
  assert.equal(readExactSource(cancelled), 'a, b, c');
  assert.equal(cancelled.doc, before.doc);
  assert.equal(cancelled.selection, before.selection);
});
/** Permission changes keep the exact stable source tree while retiring visual/session authority. */
test('native idle recipes retain inert source points through permission transitions without gesture capture', () => {
  const fixture = stateFor('a, b, c'),
    before = fixture.state,
    values = before.field(semanticStructures).native!.nativeValues,
    aggregate = nativeReorderRecipeDecorations(values);
  const readonly = before.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.ok(readonly.facet(EditorView.outerDecorations).includes(aggregate));
  assert.equal(readonly.field(reorderPreview).value, null);
  aggregate.between(0, before.doc.length, (from, to, value) => {
    if (from === to) {
      assert.equal(from, 0);
      assert.equal(value.spec.class, 'sp-reorder-native-line-0');
      assert.equal(value.spec.nativeReorderValue, '0');
      assert.equal(value.spec.widget, undefined);
      return;
    }
    assert.ok(from < to);
    assert.equal(value.spec.widget, undefined);
    assert.equal(value.spec.class.includes('active'), false);
    assert.equal(value.spec.attributes?.['data-reorder-motion-local'], undefined);
  });
  assert.equal(readExactSource(readonly), 'a, b, c');
});
/** Existing exact-source conversion supplies actual UTF-16 logical endpoints, never guessed code-point offsets. */
test('native recipe maps CRLF and astral prefixes through actual native scope mapping', () => {
  const fixture = stateFor('🧭\r\n<lora:real:0.70>, tail'),
    state = fixture.state,
    values = state.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, values.entries[0]!)!;
  const ranges: { from: number; to: number }[] = [];
  recipe.decorations.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.class === 'sp-reorder-chip') ranges.push({ from, to });
  });
  assert.ok(ranges.some((range) => range.from === 3 && range.to === 19));
  assert.equal(readExactSource(state), '🧭\r\n<lora:real:0.70>, tail');
  assert.equal(captureReorder(state, 3).prepared, recipe.prepared);
});
/** Independent TXT and CSV values retain local IDs while actual entry/scope identity separates their sources. */
for (const [kind, source] of [
  ['wildcard-txt', 'a, b\nc, d'],
  ['wildcard-csv', 'name,prompt\none,"a, b"\ntwo,"c, d"'],
] as const)
  test('native initial recipes discriminate ' + kind + ' entries', () => {
    const fixture = stateFor(source, kind),
      values = fixture.state.field(semanticStructures).native!.nativeValues;
    const recipes = values.entries.flatMap((entry) => {
      const recipe = nativeReorderRecipe(values, entry);
      return recipe ? [recipe] : [];
    });
    assert.ok(recipes.length >= 2);
    assert.equal(new Set(recipes.map((recipe) => recipe.key)).size, recipes.length);
    for (const recipe of recipes) {
      assert.equal(recipe.prepared.scope, recipe.entry.scope);
      assert.equal(nativeReorderRecipe(values, recipe.entry), recipe);
      recipe.decorations.between(0, fixture.state.doc.length, (_from, _to, value) => {
        if (_from === _to) {
          assert.equal(value.spec.nativeReorderValue, String(recipe.key));
          assert.equal(value.spec.class, 'sp-reorder-native-line-' + recipe.key);
          assert.equal(value.spec.widget, undefined);
        } else {
          assert.equal(value.spec.attributes?.['data-reorder-native-value'], String(recipe.key));
        }
      });
    }
  });

/** Supported CM comparison observes one complete stable source artifact against an independent freshly inserted long-mark control. */
test('native Alt publication changes root attributes while reusing every source decoration', () => {
  const before = stateFor('one, two, three, four').state,
    capture = captureReorder(before, 6),
    values = before.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, capture.target.entry)!;
  const value = prepareReorderPreview(
    { capture, order: capture.prepared.initial, activeChip: 1, token: {} },
    'complete',
  );
  assert.ok(value);
  const after = before.update({
      effects: installReorderPreview.of(value),
    }).state,
    beforeSets = before
      .facet(EditorView.outerDecorations)
      .filter((set): set is import('@codemirror/view').DecorationSet => typeof set !== 'function'),
    afterSets = after
      .facet(EditorView.outerDecorations)
      .filter((set): set is import('@codemirror/view').DecorationSet => typeof set !== 'function'),
    ranges: number[][] = [],
    points: number[][] = [];
  RangeSet.compare(beforeSets, afterSets, ChangeSet.empty(before.doc.length), {
    compareRange(from, to) {
      ranges.push([from, to]);
    },
    comparePoint(from, to) {
      points.push([from, to]);
    },
  });
  assert.deepEqual(ranges, []);
  assert.deepEqual(points, []);
  assert.ok(
    beforeSets.includes(recipe.decorations) ||
      beforeSets.includes(nativeReorderRecipeDecorations(values)),
  );
  assert.ok(afterSets.includes(nativeReorderRecipeDecorations(values)));
  const control: number[][] = [];
  RangeSet.compare([], [recipe.decorations], ChangeSet.empty(before.doc.length), {
    compareRange(from, to) {
      control.push([from, to]);
    },
    comparePoint() {},
  });
  assert.ok(control.some(([from, to]) => from === 5 && to === 8));
  const attributes = after
    .facet(EditorView.editorAttributes)
    .filter(
      (provider): provider is Readonly<Record<string, string>> => typeof provider !== 'function',
    );
  assert.ok(
    attributes.some(
      (row) =>
        row['data-reorder-native-value'] === String(recipe.key) &&
        row['data-reorder-native-active'] === '1' &&
        row['data-reorder-native-held'] === undefined,
    ),
  );
  const retargetedPreview = prepareReorderPreview(
    {
      capture,
      order: capture.prepared.initial,
      activeChip: 2,
      token: value.token,
    },
    'complete',
  );
  assert.ok(retargetedPreview);
  const retargeted = after.update({
    effects: installReorderPreview.of(retargetedPreview),
  }).state;
  assert.equal(
    retargeted.field(reorderPreview).decorations,
    after.field(reorderPreview).decorations,
  );
  assert.deepEqual(
    retargeted.facet(EditorView.outerDecorations),
    after.facet(EditorView.outerDecorations),
  );
  assert.equal(readExactSource(after), 'one, two, three, four');
  assert.equal(after.selection, before.selection);
});
/** The selected whole canonical replacement removes only its own native wrappers; other values and repeated exclusion outputs remain actual reused resources. */
test('canonical replacement excludes exactly one source recipe and restores the exact idle aggregate', () => {
  const before = stateFor('a, b\nc, d', 'wildcard-txt').state,
    capture = captureReorder(before, 1),
    values = before.field(semanticStructures).native!.nativeValues,
    selected = nativeReorderRecipe(values, capture.target.entry)!,
    aggregate = nativeReorderRecipeDecorations(values);
  assert.deepEqual(capture.prepared.initial.order, [0, 1]);
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  });
  assert.ok(order);
  assert.deepEqual(order.order, [1, 0]);
  const value = prepareReorderPreview({ capture, order, activeChip: 0, token: {} }, 'pointer');
  assert.ok(value);
  assert.equal(value.output.text === 'b, a', true);
  const after = before.update({
    effects: installReorderPreview.of(value),
  }).state;
  assert.equal(after.field(reorderPreview).native, false);
  const excluded = nativeReorderRecipeDecorations(values, selected.entry);
  assert.notEqual(excluded, aggregate);
  assert.equal(nativeReorderRecipeDecorations(values, selected.entry), excluded);
  assert.ok(after.facet(EditorView.outerDecorations).includes(excluded));
  const remaining: string[] = [];
  excluded.between(0, before.doc.length, (_from, _to, decoration) => {
    remaining.push(decoration.spec.nativeReorderValue);
  });
  assert.ok(remaining.length > 0);
  assert.ok(remaining.every((key) => key !== String(selected.key)));
  assert.equal(after.field(reorderPreview).decorations.size, 1);
  after
    .field(reorderPreview)
    .decorations.between(0, before.doc.length, (_from, _to, decoration) => {
      assert.equal(decoration.spec.attributes, undefined);
      assert.ok(decoration.spec.widget);
    });
  const cancelled = after.update({
    effects: installReorderPreview.of(null),
  }).state;
  assert.ok(cancelled.facet(EditorView.outerDecorations).includes(aggregate));
  assert.equal(cancelled.doc, before.doc);
  assert.equal(cancelled.selection, before.selection);
});
/** An exact source publication rebuilds only its derived recipe and never renews a previous genuine capture. */
test('source replacement prepares new native recipe identity while retaining literal current endpoints', () => {
  const before = stateFor('a, b').state,
    oldValues = before.field(semanticStructures).native!.nativeValues,
    oldRecipe = nativeReorderRecipe(oldValues, oldValues.entries[0]!)!,
    changed = initializeNativeStructure(
      before.update({ changes: { from: 0, to: 1, insert: 'long' } }).state,
    ),
    values = changed.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, values.entries[0]!)!;
  assert.notEqual(values, oldValues);
  assert.notEqual(recipe, oldRecipe);
  assert.equal(nativeReorderRecipe(values, oldValues.entries[0]!), null);
  const actual: number[][] = [];
  recipe.decorations.between(0, changed.doc.length, (from, to, decoration) => {
    if (decoration.spec.class === 'sp-reorder-chip') actual.push([from, to]);
  });
  assert.deepEqual(actual, [
    [0, 4],
    [6, 7],
  ]);
  assert.equal(readExactSource(changed), 'long, b');
});

/** Ordinary source preparation preserves every actual range beyond common engine argument limits without a product-size cap or gesture. */
test('large idle source recipe retains all independently specified chip and carrier endpoints', () => {
  const count = 70000,
    source = Array.from({ length: count }, () => 'x').join(', '),
    state = stateFor(source).state,
    values = state.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, values.entries[0]!)!;
  assert.ok(recipe);
  assert.equal(state.field(reorderPreview).value, null);
  assert.equal(recipe.prepared.chips.length, count);
  assert.equal(recipe.decorations.size, count * 2);
  assert.equal(nativeReorderRecipeDecorations(values).size, count * 2);
  assert.deepEqual(recipe.output.renderedRanges[0], { from: 0, to: 1 });
  assert.deepEqual(recipe.output.renderedRanges[count - 1], {
    from: (count - 1) * 3,
    to: (count - 1) * 3 + 1,
  });
  let chipCount = 0,
    gapCount = 0,
    lineCount = 0;
  const cursor = recipe.decorations.iter();
  for (; cursor.value; cursor.next()) {
    const from = cursor.from,
      to = cursor.to,
      decoration = cursor.value;
    if (from === to) {
      assert.equal(from, 0);
      assert.equal(decoration.spec.class, 'sp-reorder-native-line-0');
      assert.equal(decoration.spec.widget, undefined);
      lineCount++;
      continue;
    }
    if (decoration.spec.class === 'sp-reorder-chip') {
      const id = Number(decoration.spec.attributes['data-reorder-chip']);
      assert.equal(from, id * 3);
      assert.equal(to, id * 3 + 1);
      chipCount++;
    } else {
      assert.equal(decoration.spec.class, 'sp-reorder-gap');
      assert.equal(from, gapCount * 3 + 1);
      assert.equal(to, gapCount * 3 + 3);
      gapCount++;
    }
  }
  assert.deepEqual([chipCount, gapCount, lineCount], [count, count - 1, 1]);
  assert.equal(readExactSource(state) === source, true);
});
