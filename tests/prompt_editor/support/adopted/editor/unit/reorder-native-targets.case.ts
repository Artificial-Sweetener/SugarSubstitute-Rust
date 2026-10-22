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
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { setReorderMotionMask } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
/** Complete finite source decorations retain original widget and line roles through native intent without owning input or geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, ChangeSet, RangeSet } from '@codemirror/state';
import { EditorView, type Decoration, type WidgetType } from '@codemirror/view';
import { reorderState, captureReorder } from '../transaction/reorder-fixture.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  nativeReorderRecipe,
  nativeReorderRecipeDecorations,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-recipe.ts';
import {
  nativeReorderLineClass,
  nativeReorderTargetAttributes,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-targets.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionStylesFixture } from './reorder-motion-styles-fixture.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** A readonly ordered view records actual finite line/target values, not another layout or source parser. */
function points(decorations: ReturnType<typeof nativeReorderRecipeDecorations>) {
  const lines: {
      readonly from: number;
      readonly key: string;
      readonly className: string;
    }[] = [],
    targets: { readonly from: number; readonly value: Decoration }[] = [],
    cursor = decorations.iter();
  for (; cursor.value; cursor.next())
    if (cursor.from === cursor.to) {
      if (cursor.value.spec.widget) targets.push({ from: cursor.from, value: cursor.value });
      else
        lines.push({
          from: cursor.from,
          key: cursor.value.spec.nativeReorderValue,
          className: cursor.value.spec.class,
        });
    }
  return { lines, targets };
}

test('blank and multiline native source points already exist idle and native entry changes no decoration frontier', () => {
  const source = 'a, b\n\nc, d',
    before = reorderState(source).state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state,
    values = before.field(semanticStructures).native!.nativeValues,
    recipe = nativeReorderRecipe(values, values.entries[0]!)!,
    aggregate = nativeReorderRecipeDecorations(values),
    finite = points(recipe.decorations);
  assert.deepEqual(finite.lines, [
    {
      from: 0,
      key: '0',
      className: 'sp-reorder-native-line-0',
    },
    {
      from: 5,
      key: '0',
      className: 'sp-reorder-native-line-0',
    },
    {
      from: 6,
      key: '0',
      className: 'sp-reorder-native-line-0',
    },
  ]);
  assert.deepEqual(
    finite.targets.map((target) => target.from),
    [5],
  );
  assert.deepEqual(nativeReorderTargetAttributes(finite.targets[0]!.value), {
    'data-reorder-gap': '0',
    'data-reorder-blank': '0',
    'data-reorder-native-value': '0',
  });
  assert.equal(before.field(reorderPreview).value, null);
  // The actual prepared Decoration.spec record is externally untyped; its widget uses CodeMirror's public native DOM contract.
  const transport = motionPaintFixture(),
    widget: WidgetType | undefined = finite.targets[0]!.value.spec.widget;
  assert.ok(widget);
  const root = widget.toDOM(transport.view),
    target = root.querySelector<HTMLElement>('[data-reorder-gap]');
  assert.ok(target);
  assert.equal(target.style.display, 'var(--sp-reorder-native-0-target-display,none)');
  assert.equal(target.style.visibility, 'var(--sp-reorder-native-0-target-visibility,hidden)');
  assert.equal(target.style.pointerEvents, 'var(--sp-reorder-native-0-target-events,none)');
  assert.equal(target.getAttribute('data-reorder-native-value'), '0');
  const capture = captureReorder(before, 1),
    preview = prepareReorderPreview(
      { capture, order: capture.prepared.initial, activeChip: 0, token: {} },
      'complete',
    );
  assert.ok(preview);
  const entered = before.update({
    effects: installReorderPreview.of(preview),
  }).state;
  assert.equal(entered.field(reorderPreview).decorations, recipe.decorations);
  assert.ok(entered.facet(EditorView.outerDecorations).includes(aggregate));
  const differences: number[][] = [];
  RangeSet.compare(
    before
      .facet(EditorView.outerDecorations)
      .filter((set): set is import('@codemirror/view').DecorationSet => typeof set !== 'function'),
    entered
      .facet(EditorView.outerDecorations)
      .filter((set): set is import('@codemirror/view').DecorationSet => typeof set !== 'function'),
    ChangeSet.empty(before.doc.length),
    {
      compareRange(from, to) {
        differences.push([from, to]);
      },
      comparePoint(from, to) {
        differences.push([from, to]);
      },
    },
  );
  assert.deepEqual(differences, []);
  const attributes = entered
    .facet(EditorView.editorAttributes)
    .filter(
      (provider): provider is Readonly<Record<string, string>> =>
        provider !== null && typeof provider !== 'function',
    );
  assert.ok(
    attributes.some(
      (row) =>
        row['data-reorder-native-value'] === '0' &&
        row.style?.includes('--sp-reorder-native-0-target-display:inline-block') &&
        row.style.includes('--sp-reorder-native-0-target-visibility:visible') &&
        row.style.includes('--sp-reorder-native-0-target-events:auto'),
    ),
  );
  const cancelled = entered.update({
    effects: installReorderPreview.of(null),
  }).state;
  assert.ok(cancelled.facet(EditorView.outerDecorations).includes(aggregate));
  assert.equal(points(recipe.decorations).targets[0]!.value, finite.targets[0]!.value);
  assert.equal(cancelled.selection, before.selection);
  assert.equal(readExactSource(cancelled) === source, true);
});

test('CSV values sharing one physical line retain distinct source line roles and canonical exclusion removes only its actual scope', () => {
  const before = reorderState('h,k\n"a, b","c, d"', 0, 0, 'wildcard-csv').state,
    values = before.field(semanticStructures).native!.nativeValues,
    first = nativeReorderRecipe(values, values.entries[2]!)!,
    selected = nativeReorderRecipe(values, values.entries[3]!)!,
    aggregate = nativeReorderRecipeDecorations(values);
  assert.ok(first);
  assert.ok(selected);
  assert.deepEqual([first.key, selected.key], [2, 3]);
  assert.equal(first.entry.container.location.kind, 'csv');
  assert.deepEqual(points(aggregate).lines, [
    {
      from: 4,
      key: '2',
      className: 'sp-reorder-native-line-2',
    },
    {
      from: 4,
      key: '3',
      className: 'sp-reorder-native-line-3',
    },
  ]);
  assert.ok(selected.entry.semanticMapping!.logicalRange.from > 4);
  const excluded = nativeReorderRecipeDecorations(values, selected.entry);
  assert.deepEqual(points(excluded).lines, [
    {
      from: 4,
      key: '2',
      className: 'sp-reorder-native-line-2',
    },
  ]);
  assert.equal(excluded.size, first.decorations.size);
  const current = excluded.iter(),
    original = first.decorations.iter();
  for (; current.value; current.next(), original.next()) {
    assert.equal(current.value, original.value);
    assert.equal(current.from, original.from);
    assert.equal(current.to, original.to);
  }
  assert.equal(original.value, null);
  assert.equal(nativeReorderLineClass('2'), 'sp-reorder-native-line-2');
  assert.equal(nativeReorderRecipeDecorations(values, selected.entry), excluded);
  assert.equal(nativeReorderRecipeDecorations(values), aggregate);
});

/** The existing finite mask owner activates only its selected persistent native line role, preserving other values on shared rows. */
test('whole native masks require exact selected value line class and current issue gate', () => {
  const { f, owner, sheet } = motionStylesFixture(),
    causes: unknown[] = [];
  try {
    f.view.dom.setAttribute('data-reorder-native-value', '3');
    assert.equal(
      owner.prepare([0], 'whole', () => true),
      true,
    );
    const scope = reorderPaintIssueName(owner.scope);
    assert.equal(
      sheet().textContent,
      '.cm-editor[data-reorder-native-value="3"][data-reorder-motion-scope="' +
        scope +
        '"] > [data-reorder-motion-gate="' +
        scope +
        '"][data-reorder-motion-active="' +
        scope +
        '"] ~ .cm-scroller > .cm-content .sp-reorder-native-line-3{opacity:0!important}',
    );
    assert.equal(sheet().textContent!.includes('.sp-reorder-pointer-underlay'), false);
    assert.equal(sheet().textContent!.includes('.sp-reorder-native-line-2'), false);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.retire();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'native-line-role-retirement-failed');
});

/** Actual multi-value recipes and canonical whole coverage keep the generic underlay selector exclusive to the changed scope. */
test('canonical whole masks cannot select persistent foreign native lines', () => {
  const source = 'a, b\nc, d',
    before = reorderState(source, 0, 0, 'wildcard-txt').state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state,
    values = before.field(semanticStructures).native!.nativeValues,
    original = nativeReorderRecipeDecorations(values),
    capture = captureReorder(before, 1),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
  assert.deepEqual(points(original).lines, [
    { from: 0, key: '0', className: 'sp-reorder-native-line-0' },
    { from: 5, key: '1', className: 'sp-reorder-native-line-1' },
  ]);
  assert.ok(order);
  assert.deepEqual(order.order, [1, 0]);
  const preview = prepareReorderPreview({ capture, order, activeChip: 0, token: {} }, 'pointer');
  assert.ok(preview);
  assert.equal(preview.output.text === 'b, a', true);
  const changed = before.update({
      effects: installReorderPreview.of(preview),
    }).state,
    { f, owner, sheet } = motionStylesFixture(),
    causes: unknown[] = [];
  try {
    const masked = changed.update({
        effects: setReorderMotionMask.of({
          capture,
          issue: {},
          scope: owner.scope,
          ids: [],
          coverage: 'value',
        }),
      }).state,
      publication = masked.field(reorderPreview),
      remaining = nativeReorderRecipeDecorations(values, capture.target.entry),
      attrs = masked
        .facet(EditorView.editorAttributes)
        .filter(
          (p): p is Readonly<Record<string, string>> => p !== null && typeof p !== 'function',
        );
    assert.equal(publication.native, false);
    assert.equal(publication.mask?.coverage, 'value');
    assert.ok(attrs.every((row) => row['data-reorder-native-value'] === undefined));
    for (const row of attrs)
      for (const [name, value] of Object.entries(row)) f.view.dom.setAttribute(name, value);
    assert.equal(
      owner.prepare([], 'whole', () => true),
      true,
    );
    const scope = reorderPaintIssueName(owner.scope);
    assert.equal(
      sheet().textContent,
      '.cm-editor > [data-reorder-motion-gate="' +
        scope +
        '"][data-reorder-motion-active="' +
        scope +
        '"] ~ .cm-scroller > .cm-content .sp-reorder-pointer-underlay{opacity:0!important}',
    );
    const canonical = publication.outer.iter();
    assert.ok(canonical.value);
    assert.deepEqual(
      [canonical.from, canonical.to, canonical.value.spec.class],
      [0, 4, 'sp-reorder-pointer-underlay'],
    );
    const canonicalNode = f.document.createElement('span');
    canonicalNode.className = canonical.value.spec.class;
    canonical.next();
    assert.equal(canonical.value, null);
    assert.deepEqual(points(remaining).lines, [
      { from: 5, key: '1', className: 'sp-reorder-native-line-1' },
    ]);
    const foreign = f.document.createElement('div');
    foreign.className = 'cm-line ' + points(remaining).lines[0]!.className;
    f.content.replaceChildren(canonicalNode, foreign);
    assert.deepEqual(f.content.querySelectorAll('.sp-reorder-pointer-underlay'), [canonicalNode]);
    assert.deepEqual(f.content.querySelectorAll('.sp-reorder-native-line-1'), [foreign]);
    assert.ok(masked.facet(EditorView.outerDecorations).includes(remaining));
    assert.equal(masked.doc, before.doc);
    assert.equal(masked.selection, before.selection);
    assert.equal(readExactSource(masked) === source, true);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.retire();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length)
    throw new AggregateError(causes, 'canonical-mask-line-isolation-retirement-failed');
});
