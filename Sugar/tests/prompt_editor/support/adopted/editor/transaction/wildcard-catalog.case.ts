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
/** Native state proves catalog-bound projection publication, source preservation and actual reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { projectionState } from './projection-state.ts';

test('catalog changes reject prior prepared projection/context and reuse current rendering across directional selection', () => {
  const compartment = new Compartment(),
    catalog = prepareWildcardCatalog({
      status: 'ready',
      revision: 'ready',
      textIdentifiers: ['animal'],
      csvSources: [],
    });
  let state = projectionState('🙂\r\n{animal}, {absent}').update({
    effects: StateEffect.appendConfig.of(compartment.of(wildcardCatalog.of(catalog))),
  }).state;
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(prepared) }).state;
  const all = state.field(richProjection).all,
    freshness = state.field(editorFreshness);
  state = state.update({
    selection: { anchor: state.doc.length, head: 3 },
  }).state;
  assert.equal(state.field(richProjection).all, all);
  assert.equal(state.field(richProjection).wildcardCatalog, catalog);
  state = state.update({
    effects: [
      compartment.reconfigure(
        wildcardCatalog.of(prepareWildcardCatalog({ status: 'pending', revision: 'pending' })),
      ),
      invalidateCommandContext.of(null),
    ],
  }).state;
  assert.equal(state.field(editorFreshness).source, freshness.source);
  assert.notEqual(state.field(editorFreshness).context, freshness.context);
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(prepared) }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  const pending = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(pending) }).state;
  const statuses: string[] = [];
  pending.all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.attributes) statuses.push(String(value.spec.attributes['data-resolution']));
  });
  assert.deepEqual(statuses, ['pending', 'pending']);
  assert.equal(undoDepth(state), 0);
  assert.equal(state.doc.toString(), '🙂\n{animal}, {absent}');
  assert.equal(state.selection.main.anchor, state.doc.length);
  assert.equal(state.selection.main.head, 3);
});

test('raw catalog replacement retains empty decorations and current identity', () => {
  const presentation = new Compartment(),
    catalog = new Compartment();
  let state = projectionState('{animal}', presentation).update({
    effects: [
      presentation.reconfigure(richRendering.of(false)),
      StateEffect.appendConfig.of(catalog.of(wildcardCatalog.of(null))),
    ],
  }).state;
  state = state.update({
    effects: catalog.reconfigure(
      wildcardCatalog.of(
        prepareWildcardCatalog({
          status: 'ready',
          revision: 'empty',
          textIdentifiers: [],
          csvSources: [],
        }),
      ),
    ),
  }).state;
  assert.equal(state.field(richProjection).all.size, 0);
  assert.equal(state.field(richProjection).sourceIdentity, state.field(editorFreshness).source);
  assert.equal(undoDepth(state), 0);
});
