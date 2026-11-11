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
import { weightStateFor } from './weight-command-state.ts';
/** Native semantic targets and source/history transactions qualify region ownership without simulated DOM. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, StateEffect, Transaction } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  currentRegionName,
  regionNameTransactions,
} from '../../../../../../frontend/sugarprompt/editor/src/region-command.ts';
import { regionAtPosition } from '../../../../../../frontend/sugarprompt/editor/src/region-index.ts';
import {
  regionDraft,
  setRegionDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/region-draft.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';

/** Publish actual prepared semantic output after native state creation. */
function prepared(source: string, presentation = new Compartment()): EditorState {
  const state = projectionState(source, presentation);
  return state.update({
    effects: [
      StateEffect.appendConfig.of(regionDraft),
      installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(state),
      }),
    ],
  }).state;
}

test('bulk-prepared logical targets reuse actual output across selection; exact rename commits once and retires its source intent', () => {
  const source = '🦊\r\n[SEP|Face]\rbody\n';
  let state = prepared(source);
  const regions = state.field(semanticStructures).native!.regions;
  const target = regions[0]!;
  assert.equal(target.from, source.indexOf('[SEP') - 1);
  assert.equal(regionAtPosition(regions, target.from), target);
  assert.equal(regionAtPosition(regions, target.to), target);
  assert.equal(regionAtPosition(regions, target.to + 1), null);
  const intent = { target, freshness: state.field(editorFreshness) };
  state = state.update({
    selection: { anchor: state.doc.length, head: target.from },
  }).state;
  assert.equal(state.field(semanticStructures).native!.regions, regions);
  for (const transaction of regionNameTransactions(state, intent, '背景 🦊'))
    state = transaction.state;
  assert.equal(readExactSource(state) === source.replace('Face', '背景 🦊'), true);
  assert.equal(undoDepth(state), 1);
  assert.equal(currentRegionName(state, intent), false);
  assert.equal(regionNameTransactions(state, intent, 'obsolete').length, 0);
});

test('no-op drafts keep native history empty; raw mode, read-only and source edits reject captured commands', () => {
  const presentation = new Compartment();
  let state = prepared('[SEP|Face]\r\nbody', presentation);
  const target = state.field(semanticStructures).native!.regions[0]!;
  const intent = { target, freshness: state.field(editorFreshness) };
  for (const transaction of regionNameTransactions(state, intent, 'Face'))
    state = transaction.state;
  assert.equal(undoDepth(state), 0);
  assert.equal(regionNameTransactions(state, intent, ']').length, 0);
  const raw = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  assert.equal(regionNameTransactions(raw, intent, 'changed').length, 0);
  const readOnly = state.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(regionNameTransactions(readOnly, intent, 'changed').length, 0);
  state = state.update(
    exactReplacement(state, state.doc.length + 1, state.doc.length + 1, 'x'),
  ).state;
  assert.equal(regionNameTransactions(state, intent, 'changed').length, 0);
});

test('source-free draft geometry reuses its actual value on selection and cannot revive after source or permissions retire it', () => {
  let state = prepared('[SEP|Face]\nbody');
  const value = {
    freshness: state.field(editorFreshness),
    scopeId: state.field(semanticStructures).native!.regions[0]!.scopeId,
    ordinal: 0,
    width: 36,
  };
  state = state.update({ effects: setRegionDraft.of(value) }).state;
  state = state.update({ selection: { anchor: 2 } }).state;
  assert.equal(state.field(regionDraft), value);
  state = state.update(exactReplacement(state, state.doc.length, state.doc.length, 'x')).state;
  assert.equal(state.field(regionDraft), null);
  state = state.update({ effects: setRegionDraft.of(value) }).state;
  assert.equal(state.field(regionDraft), null);
  state = state.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  state = state.update({
    effects: setRegionDraft.of({
      ...value,
      freshness: state.field(editorFreshness),
    }),
  }).state;
  assert.equal(state.field(regionDraft), null);
});

test('ordinal hover ranges reuse actual prepared projection and never attach an empty partition rail to the next structural row', () => {
  let state = prepared('[SEP]\n[SEP|Face]\nbody\n[SEP]\n');
  const semantic = state.field(semanticStructures).native!;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, semantic)),
  }).state;
  const hovers = state.field(richProjection).regionHovers[semantic.regions[0]!.scopeId]!;
  assert.equal(hovers.length, 3);
  assert.equal(hovers[0]!.decorations.size, 1);
  assert.equal(hovers[1]!.decorations.size, 2);
  const rails: number[] = [];
  state.field(richProjection).all.between(0, state.doc.length, (from, _to, decoration) => {
    if (decoration.spec.attributes?.class === 'sp-region-content') rails.push(from);
  });
  assert.equal(rails.includes(semantic.regions[1]!.from), false);
  state = state.update({
    selection: { anchor: state.doc.length, head: 0 },
  }).state;
  assert.equal(state.field(richProjection).regionHovers[semantic.regions[0]!.scopeId], hovers);
});

test('CSV scope-local region names encode quotes/comma/Unicode, conserve adjacent ordinal-zero regions and retain current populated seeds/history', () => {
  const source = 'H,Z\r\n"[SEP|A]\r\n🙂 body","[SEP|B]\r\nneighbor"\rtail,last\n';
  let state = weightStateFor(source, 0, 0, 'wildcard-csv');
  state = state.update({
    effects: StateEffect.appendConfig.of(regionDraft),
  }).state;
  const native = state.field(semanticStructures).native!,
    first = native.regions[0]!,
    second = native.regions[1]!;
  assert.equal(first.ordinal, 0);
  assert.equal(second.ordinal, 0);
  assert.notEqual(first.scopeId, second.scopeId);
  const intent = { target: second, freshness: state.field(editorFreshness) };
  assert.equal(currentRegionName(state, intent), true);
  assert.equal(currentRegionName(state, { ...intent, target: { ...second } }), false);
  assert.equal(
    currentRegionName(state, {
      ...intent,
      target: { ...second, scopeId: first.scopeId },
    }),
    false,
  );
  const value = {
    freshness: intent.freshness,
    scopeId: second.scopeId,
    ordinal: 0,
    width: 33,
  };
  state = state.update({
    selection: { anchor: state.doc.length, head: second.from },
    effects: setRegionDraft.of(value),
  }).state;
  const originalSelection = state.selection;
  const transactions = regionNameTransactions(state, intent, 'Q", 🙂');
  assert.equal(transactions.length, 2);
  assert.equal(transactions[0]!.annotation(Transaction.userEvent), 'input.region');
  state = transactions.at(-1)!.state;
  assert.equal(readExactSource(state) === source.replace('[SEP|B]', '[SEP|Q"", 🙂]'), true);
  const current = state.field(semanticStructures).native!;
  assert.equal(current.regions[0]!.separator.name, 'A');
  assert.equal(current.regions[1]!.separator.name === 'Q", 🙂', true);
  assert.equal(current.nativeValues.scopes.values, current.documentSemantics.preparedValues);
  assert.equal(prepareNativeStructure(state), current);
  assert.equal(state.field(richProjection).sourceIdentity, state.field(editorFreshness).source);
  assert.equal(currentRegionName(state, intent), false);
  assert.equal(regionNameTransactions(state, intent, 'old').length, 0);
  assert.equal(undoDepth(state), 1);
  const changedSelection = state.selection;
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state) === source, true);
  assert.equal(state.selection.eq(originalSelection), true);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(state.selection.eq(changedSelection), true);
  state = transactions.at(-1)!.state;
  const same = regionNameTransactions(
    state,
    { target: current.regions[1]!, freshness: state.field(editorFreshness) },
    'Q", 🙂',
  ).at(-1)!.state;
  assert.equal(same.field(semanticStructures).native, current);
  assert.equal(same.field(semanticStructures).native!.nativeValues, current.nativeValues);
  assert.equal(
    same.field(semanticStructures).native!.scopes.scopes[1]!.document.regions.separators.length,
    1,
  );
  assert.equal(undoDepth(same), 1);
});
