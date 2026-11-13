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
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
/** Actual canonical projection/origin mapping proves shared paint identities without claiming native raster parity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, EditorState, EditorSelection } from '@codemirror/state';
import { type DecorationSet } from '@codemirror/view';
import {
  serializeValueReorder,
  moveReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { reorderChipSource } from '../../../../../../frontend/sugarprompt/core/src/reorder/serialization.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  installProjection,
  richProjection,
  currentSourceDisplayProjection,
  revealProjectionToken,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  currentReorderProjection,
  projectReorderOutput,
  projectReorderText,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import {
  SourceWidget,
  EmphasisWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { projectionState } from './projection-state.ts';
import { prepareReorderCommit } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { preparedReorderSourceLines } from '../../../../../../frontend/sugarprompt/editor/src/source-line-origins.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  installReorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  holdProjectionToken,
  releaseProjectionToken,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { captureReorder } from './reorder-fixture.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  installLoraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { installLoraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import {
  installSceneErrors,
  prepareSceneErrors,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';

/** Publish the real canonical compilation over initialized values before genuine command capture. */
function painted(source: string, kind: 'prompt' | 'wildcard-csv' = 'prompt'): EditorState {
  const state = initializeNativeStructure(projectionState(source, undefined, kind));
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}
/** Observe original widget identities in actual native decoration order. */
function widgets(decorations: DecorationSet, length: number): readonly SourceWidget[] {
  const result: SourceWidget[] = [];
  decorations.between(0, length, (_from, _to, value) => {
    if (value.spec.widget instanceof SourceWidget) result.push(value.spec.widget);
  });
  return result;
}

test('reordered shared emphasis reopens original chrome around separated children and isolated LoRA proxy', () => {
  const state = painted('(a, <lora:model:0.50>:2.00), tail').update({
      selection: { anchor: 1 },
    }).state,
    capture = captureReorder(state);
  assert.ok(capture.projection);
  const canonical = widgets(state.field(richProjection).all, state.doc.length);
  const prefix = canonical.find((w) => w instanceof EmphasisWidget && w.part === 'prefix')!;
  const suffix = canonical.find((w) => w instanceof EmphasisWidget && w.part === 'suffix')!;
  const lora = canonical.find((w) => w instanceof LoraWidget)!;
  assert.equal(canonical.length, 3);
  const initial = serializeValueReorder(capture.prepared)!;
  assert.deepEqual(
    widgets(projectReorderOutput(capture.projection, initial), initial.text.length),
    [prefix, lora, suffix],
  );
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  const moved = serializeValueReorder(capture.prepared, order)!;
  assert.deepEqual(widgets(projectReorderOutput(capture.projection, moved), moved.text.length), [
    prefix,
    lora,
    suffix,
    prefix,
    suffix,
  ]);
  const chip = capture.prepared.chips.find((c) => c.text.startsWith('<lora:'))!;
  const proxy = reorderChipSource(chip);
  assert.equal(proxy.text, '(<lora:model:0.50>:2.00)');
  assert.deepEqual(widgets(projectReorderText(capture.projection, proxy), proxy.text.length), [
    prefix,
    lora,
    suffix,
  ]);
  assert.equal(currentReorderProjection(state, capture.projection), true);
  const changed = state.update(exactReplacement(state, 1, 2, 'changed')).state;
  assert.equal(currentReorderProjection(changed, capture.projection), false);
});

test('structured CRLF storage maps rich widgets to decoded value and leaves CSV encoding outside paint', () => {
  const state = painted('h,k\r\n"(🙂, <lora:model:0.50>:2.00)",keep', 'wildcard-csv');
  const capture = captureReorder(state, state.doc.toString().indexOf('🙂'));
  assert.ok(capture.projection);
  const output = serializeValueReorder(capture.prepared)!;
  assert.equal(output.text, '(🙂, <lora:model:0.50>:2.00)');
  assert.deepEqual(
    widgets(projectReorderOutput(capture.projection, output), output.text.length),
    widgets(state.field(richProjection).all, state.doc.length),
  );
});

test('scene title marks and concealed marker reuse structural ownership even when title resembles syntax', () => {
  const state = painted('**(title:2.00)\na, b'),
    capture = captureReorder(state);
  assert.ok(capture.projection);
  const output = serializeValueReorder(capture.prepared)!;
  const mapped = projectReorderOutput(capture.projection, output);
  const classes: unknown[] = [];
  mapped.between(0, output.text.length, (_from, _to, value) => {
    if (value.spec.class) classes.push(value.spec.class);
  });
  assert.ok(classes.includes('sp-scene-title'));
  assert.equal(
    widgets(mapped, output.text.length).some((w) => w instanceof EmphasisWidget),
    false,
  );
  assert.deepEqual(
    widgets(mapped, output.text.length),
    widgets(state.field(richProjection).all, state.doc.length),
  );
});

test('same-source external paint changes retire the captured canonical compilation before reorder publication', () => {
  const state = painted('**title\n<lora:model:0.50>, {animal|2}'),
    capture = captureReorder(state),
    identity = state.field(editorFreshness).source;
  assert.ok(capture.projection);
  const publications = [
    installLoraPresentation.of(prepareLoraPresentation(state, { status: 'ready', resources: [] })),
    installLoraMedia.of({
      freshness: state.field(editorFreshness),
      media: new Map(),
    }),
    installSceneErrors.of(prepareSceneErrors(identity, ['title'])),
    StateEffect.appendConfig.of(
      wildcardCatalog.of(prepareWildcardCatalog({ status: 'pending', revision: 'changed' })),
    ),
  ];
  for (const effect of publications) {
    const changed = state.update({ effects: effect }).state;
    assert.equal(changed.doc, state.doc);
    assert.equal(changed.field(editorFreshness).source, identity);
    assert.equal(currentReorderProjection(changed, capture.projection), false);
  }
});

test('selected multiline LoRA source remains visible through moved origins and actual mapped commit selection', () => {
  const source = '<lora:model\nname:0.50>, b, tail\nfollowing',
    expected = 'b, <lora:model\nname:0.50>, tail\nfollowing',
    state = painted(source).update({
      selection: { anchor: 8 },
      effects: StateEffect.appendConfig.of(authorshipExtension),
    }).state,
    capture = captureReorder(state),
    projection = capture.projection!;
  assert.equal(widgets(projection.canonical, state.doc.length).length, 1);
  assert.deepEqual(widgets(projection.visible, state.doc.length), []);
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    output = serializeValueReorder(capture.prepared, order)!;
  assert.equal(output.text, expected);
  assert.deepEqual(widgets(projectReorderOutput(projection, output), output.text.length), []);
  assert.equal(preparedReorderSourceLines(capture, output).length, 3);
  const result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  if (result.status !== 'ready') throw new Error('source-display-commit-unavailable');
  const committed = result.edit.transactions.at(-1)!.state;
  assert.equal(committed.doc.toString(), expected);
  assert.deepEqual([committed.selection.main.anchor, committed.selection.main.head], [11, 11]);
  const independentlyPainted = painted(expected).update({
    selection: committed.selection,
  }).state;
  assert.deepEqual(
    widgets(
      currentSourceDisplayProjection(independentlyPainted)!.decorations,
      independentlyPainted.doc.length,
    ),
    [],
  );
});

test('unchanged reorder metadata retains display provenance while explicit reveal retires the same compilation', () => {
  const state = painted('<lora:model:0.50>, b').update({
      selection: { anchor: 8 },
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state,
    capture = captureReorder(state),
    preview = prepareReorderPreview(
      {
        token: {},
        capture,
        activeChip: capture.active,
        order: capture.prepared.initial,
      },
      'complete',
    )!,
    published = state.update({
      effects: installReorderPreview.of(preview),
    }).state;
  assert.equal(published.field(richProjection), state.field(richProjection));
  assert.equal(currentReorderProjection(published, capture.projection), true);
  const revealed = published.update({
    effects: revealProjectionToken.of({
      sourceIdentity: state.field(editorFreshness).source,
      range: { from: 0, to: 17 },
    }),
  }).state;
  assert.equal(revealed.field(richProjection).all, state.field(richProjection).all);
  assert.notEqual(revealed.field(richProjection), state.field(richProjection));
  assert.equal(currentReorderProjection(revealed, capture.projection), false);
});

test('native held exposure and its release retire captured readonly visibility without replacing canonical widgets', () => {
  const state = painted('<lora:model:0.50>, b'),
    capture = captureReorder(state),
    session = Symbol(),
    held = state.update({
      effects: holdProjectionToken.of({
        session,
        freshness: state.field(editorFreshness),
        range: { from: 0, to: 17 },
        prepared: state.field(richProjection),
      }),
    }).state;
  assert.equal(held.field(richProjection).all, state.field(richProjection).all);
  assert.equal(currentReorderProjection(held, capture.projection), false);
  const heldCapture = captureReorder(held);
  assert.deepEqual(widgets(heldCapture.projection!.visible, held.doc.length), []);
  const released = held.update({
    effects: releaseProjectionToken.of(session),
  }).state;
  assert.equal(released.field(richProjection).all, held.field(richProjection).all);
  assert.equal(currentReorderProjection(released, heldCapture.projection), false);
  assert.equal(
    widgets(currentSourceDisplayProjection(released)!.decorations, released.doc.length).length,
    1,
  );
});

for (const kind of ['prompt', 'wildcard-csv'] as const)
  test(`selected emphasis suffix delegates complete source exposure through actual ${kind} coordinates`, () => {
    const source =
      kind === 'prompt' ? '("a":1.20), b' : 'name,prompt\r\nrow,"(""a"":1.20), b"\r\nlast,end';
    let state = painted(source, kind);
    state = state.update({
      selection: { anchor: state.doc.toString().indexOf(':1.20') + 3 },
    }).state;
    const capture = captureReorder(state),
      output = serializeValueReorder(capture.prepared)!;
    assert.equal(widgets(capture.projection!.canonical, state.doc.length).length, 2);
    assert.deepEqual(widgets(capture.projection!.visible, state.doc.length), []);
    assert.deepEqual(
      widgets(projectReorderOutput(capture.projection, output), output.text.length),
      [],
    );
    assert.deepEqual(
      widgets(currentSourceDisplayProjection(state)!.decorations, state.doc.length),
      [],
    );
  });

test('readonly display consumes every actual native selection range without a parallel exposure rule', () => {
  let state = painted('<lora:first:0.50>, <lora:second:0.75>, tail');
  state = state.update({
    effects: StateEffect.appendConfig.of(EditorState.allowMultipleSelections.of(true)),
  }).state;
  const second = state.doc.toString().indexOf('second') + 2;
  state = state.update({
    selection: EditorSelection.create([EditorSelection.cursor(8), EditorSelection.cursor(second)]),
  }).state;
  assert.equal(state.selection.ranges.length, 2);
  const capture = captureReorder(state),
    projection = capture.projection!;
  assert.equal(widgets(projection.canonical, state.doc.length).length, 2);
  assert.deepEqual(widgets(projection.visible, state.doc.length), []);
});
