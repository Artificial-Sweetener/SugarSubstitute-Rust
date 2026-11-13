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
/** Source-current numeric intent projects one cell without changing canonical widgets, source, selection or history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';

/** Publish real prepared semantic output with the native numeric session owner installed. */
function prepared(source: string, presentation = new Compartment()): EditorState {
  const state = projectionState(source, presentation).update({
    effects: StateEffect.appendConfig.of(weightSession),
    selection: { anchor: 2, head: 1 },
  }).state;
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

/** Read the actual projected numeric widgets independently of the projection implementation. */
function cells(state: EditorState): readonly (EmphasisWidget | LoraWidget)[] {
  const result: (EmphasisWidget | LoraWidget)[] = [];
  state.field(richProjection).decorations.between(0, state.doc.length, (_from, _to, decoration) => {
    const widget = decoration.spec.widget;
    if (
      widget instanceof LoraWidget ||
      (widget instanceof EmphasisWidget && widget.part === 'suffix')
    )
      result.push(widget);
  });
  return result;
}

for (const kind of ['emphasis', 'lora'] as const) {
  test(`${kind} native intent reuses canonical output and only replaces its current numeric cell`, () => {
    const source = '(cat:2.00), <lora:real:0.75>\r\nnext';
    let state = prepared(source);
    const canonical = state.field(richProjection),
      original = cells(state);
    const intent = {
      kind,
      range:
        kind === 'emphasis'
          ? { from: 1, to: 4 }
          : { from: source.indexOf('<'), to: source.indexOf('>') + 1 },
      freshness: state.field(editorFreshness),
      key: Symbol(),
      phase: 'editing' as const,
    };
    state = state.update({ effects: setExactWeightDraft.of(intent) }).state;
    const opened = cells(state);
    assert.deepEqual(
      opened.map((value) => value.draftKey),
      kind === 'emphasis' ? [intent.key, null] : [null, intent.key],
    );
    assert.equal(opened[kind === 'emphasis' ? 1 : 0], original[kind === 'emphasis' ? 1 : 0]);
    assert.equal(state.field(richProjection).all, canonical.all);
    assert.equal(state.field(richProjection).allAtomic, canonical.allAtomic);
    assert.equal(state.field(richProjection).tokens, canonical.tokens);
    assert.equal(readExactSource(state) === source, true);
    assert.deepEqual(
      [state.selection.main.anchor, state.selection.main.head, undoDepth(state)],
      [2, 1, 0],
    );
    const active = state.field(richProjection);
    state = state.update({ selection: { anchor: state.doc.length } }).state;
    assert.equal(state.field(exactWeightDraft), intent);
    assert.equal(state.field(richProjection), active);
    state = state.update({ effects: setExactWeightDraft.of(null) }).state;
    assert.deepEqual(
      cells(state).map((value) => value.draftKey),
      [null, null],
    );
    assert.equal(state.field(richProjection).all, canonical.all);
    assert.equal(undoDepth(state), 0);
  });
}

test('native numeric intent cannot revive after source, context, permission or raw-presentation retirement', () => {
  const source = '(cat:2.00)';
  const presentation = new Compartment();
  const initial = prepared(source, presentation);
  const draft = {
    kind: 'emphasis',
    range: { from: 1, to: 4 },
    freshness: initial.field(editorFreshness),
    key: Symbol(),
    phase: 'editing' as const,
  } as const;
  const opened = initial.update({
    effects: setExactWeightDraft.of(draft),
  }).state;
  const changed = opened.update(exactReplacement(opened, source.length, source.length, 'x')).state;
  const restored = changed.update(
    exactReplacement(changed, source.length, source.length + 1, ''),
  ).state;
  for (const retired of [
    restored,
    opened.update({ effects: invalidateCommandContext.of(null) }).state,
    opened.update({
      effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
    }).state,
    opened.update({
      effects: presentation.reconfigure(richRendering.of(false)),
    }).state,
  ]) {
    assert.equal(retired.field(exactWeightDraft), null);
    assert.equal(
      retired.update({ effects: setExactWeightDraft.of(draft) }).state.field(exactWeightDraft),
      null,
    );
    assert.equal(readExactSource(retired) === source, true);
  }
});

test('awaiting source publication preserves the current keyed widget contract and canonical projection', () => {
  let state = prepared('(cat:1.20), tail');
  const draft = {
    kind: 'emphasis',
    range: { from: 1, to: 4 },
    freshness: state.field(editorFreshness),
    key: Symbol('draft'),
    phase: 'editing',
  } as const;
  state = state.update({ effects: setExactWeightDraft.of(draft) }).state;
  const editing = cells(state)[0]!,
    canonical = state.field(richProjection);
  state = state.update({
    effects: setExactWeightDraft.of({ ...draft, phase: 'awaiting-commit' }),
  }).state;
  assert.equal(cells(state)[0]!.draftKey, draft.key);
  const awaiting = cells(state)[0]!;
  assert.ok(editing instanceof EmphasisWidget);
  assert.ok(awaiting instanceof EmphasisWidget);
  assert.equal(awaiting.eq(editing), true);
  assert.equal(state.field(richProjection).all, canonical.all);
  assert.equal(readExactSource(state), '(cat:1.20), tail');
  assert.equal(undoDepth(state), 0);
});
