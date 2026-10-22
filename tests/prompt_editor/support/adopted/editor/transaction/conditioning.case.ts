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
/** Native fields admit only current host-classified topology and reuse actual semantic output across selection. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect } from '@codemirror/state';
import { projectionState } from './projection-state.ts';
import { conditioningContext } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-context.ts';
import {
  nativeConditioning,
  prepareConditioning,
  installConditioning,
} from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { captureConditioningContext } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  freshnessRejection,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';

test('host topology changes reuse parsed source while old derived values cannot publish into replaced authority', () => {
  const host = new Compartment(),
    presentation = new Compartment();
  const source = '🦊 global\r\n[SEP|Face]\rbody\n[SEP]\n';
  let state = projectionState(source, presentation).update({
    effects: StateEffect.appendConfig.of([
      nativeConditioning,
      host.of(conditioningContext.of(captureConditioningContext({ key: 'a', mode: 'regional' }))),
    ]),
  }).state;
  const semantic = prepareNativeStructure(state);
  const first = prepareConditioning(state, semantic);
  state = state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installConditioning.of(first),
    ],
  }).state;
  const freshness = state.field(editorFreshness);
  for (const anchor of [0, 2, state.doc.length])
    state = state.update({ selection: { anchor } }).state;
  assert.equal(prepareConditioning(state, prepareNativeStructure(state)), first);
  assert.equal(
    first.topology.effectiveRanges('partition:1'),
    first.topology.effectiveRanges('partition:1'),
  );
  assert.equal(first.topology.children('partition:0'), first.topology.emitted);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  assert.equal(prepareNativeStructure(state), semantic);
  assert.equal(prepareConditioning(state, semantic), first);
  state = state.update({
    effects: host.reconfigure(
      conditioningContext.of(captureConditioningContext({ key: 'b', mode: 'independent' })),
    ),
  }).state;
  assert.equal(freshnessRejection(state.field(editorFreshness), freshness), 'stale-context');
  assert.equal(prepareNativeStructure(state), semantic);
  assert.equal(state.field(nativeConditioning), null);
  state = state.update({ effects: installConditioning.of(first) }).state;
  assert.equal(state.field(nativeConditioning), null);
  const second = prepareConditioning(state, semantic);
  assert.equal(second.topology.emitted.length, 3);
  assert.equal(second.topology.partition(1).parent, null);
  assert.equal(readExactSource(state) === source, true);
  assert.equal(state.selection.main.head, state.doc.length);
  state = state.update(exactReplacement(state, source.length, source.length, 'x')).state;
  assert.equal(freshnessRejection(state.field(editorFreshness), freshness), 'stale-source');
  assert.throws(() => prepareConditioning(state, semantic), /Stale conditioning source/u);
});

test('candidate meaning cannot admit a prepared prompt topology even when the source identity matches', () => {
  let state = projectionState('[SEP]\n').update({
    effects: StateEffect.appendConfig.of(nativeConditioning),
  }).state;
  const prepared = prepareConditioning(state, prepareNativeStructure(state));
  state = state.update({
    effects: StateEffect.appendConfig.of(documentKind.of('wildcard-txt')),
  }).state;
  state = state.update({ effects: installConditioning.of(prepared) }).state;
  assert.equal(state.field(nativeConditioning), null);
});
