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
/** Independent native facet controls establish regional observation completeness without browser qualification. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, RangeSet, StateEffect } from '@codemirror/state';
import { Decoration, EditorView } from '@codemirror/view';
import { regionDraftDecorationSets } from '../transaction/region-draft-observation.ts';
import { hostFixture } from '../transaction/host-document-fixture.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import {
  regionDraft,
  setRegionDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/region-draft.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { RegionWidget } from '../../../../../../frontend/sugarprompt/editor/src/region-widget.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';

/** Read literal classes from actual provided ranges, never a synthetic decoration counter. */
function classes(state: EditorState, view?: EditorView): string[] {
  const values: string[] = [];
  for (const set of regionDraftDecorationSets(state, view))
    set.between(0, state.doc.length, (_from, _to, value) => {
      const name = value.spec.attributes?.class;
      if (typeof name === 'string') values.push(name);
    });
  return values;
}

test('regional observation conserves ordinary and outer ranges while the former ordinary-only observer omits an actual range', () => {
  const ordinary = Decoration.mark({ attributes: { class: 'ordinary' } }).range(1, 3);
  const outer = Decoration.mark({ attributes: { class: 'outer' } }).range(0, 4);
  const state = EditorState.create({
    doc: 'abcd',
    extensions: [
      EditorView.decorations.of(Decoration.set([ordinary])),
      EditorView.outerDecorations.of(Decoration.set([outer])),
    ],
  });
  assert.deepEqual(classes(state), ['ordinary', 'outer']);
  const former: string[] = [];
  for (const provider of state.facet(EditorView.decorations)) {
    assert.notEqual(typeof provider, 'function');
    if (typeof provider !== 'function')
      provider.between(0, 4, (_from, _to, value) => {
        former.push(value.spec.attributes.class);
      });
  }
  assert.deepEqual(former, ['ordinary']);
});

test('dynamic ordinary and outer providers receive only the current existing native fixture view', () => {
  const host = hostFixture(),
    view = host.transport as unknown as EditorView;
  const ordinary = Decoration.set([
    Decoration.mark({ attributes: { class: 'dynamic-ordinary' } }).range(0, 1),
  ]);
  const outer = Decoration.set([
    Decoration.mark({ attributes: { class: 'dynamic-outer' } }).range(0, 1),
  ]);
  host.transport.dispatch({
    effects: StateEffect.appendConfig.of([
      EditorView.decorations.of((actual) => {
        assert.equal(actual, view);
        return ordinary;
      }),
      EditorView.outerDecorations.of((actual) => {
        assert.equal(actual, view);
        return outer;
      }),
    ]),
  });
  assert.equal(classes(host.state(), view).includes('dynamic-ordinary'), true);
  assert.equal(classes(host.state(), view).includes('dynamic-outer'), true);
  assert.throws(() => regionDraftDecorationSets(host.state()), /provider requires view/u);
  assert.throws(
    () => regionDraftDecorationSets(host.state().update({ selection: { anchor: 1 } }).state, view),
    /state mismatch/u,
  );
});

test('actual scoped regional draft is an outer active mark around its actual ordinary replacement widget', () => {
  let state = weightStateFor('header,x\n"[SEP|A]",body', 0, 0, 'wildcard-csv');
  state = state.update({
    effects: StateEffect.appendConfig.of(regionDraft),
  }).state;
  const native = state.field(semanticStructures).native!;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, native)),
  }).state;
  const target = native.regions[0]!;
  state = state.update({
    effects: setRegionDraft.of({
      freshness: state.field(editorFreshness),
      scopeId: target.scopeId,
      ordinal: target.ordinal,
      width: 33,
    }),
  }).state;
  let wrapped = false;
  RangeSet.spans(regionDraftDecorationSets(state), target.from, target.to, {
    span() {},
    point(from, to, value, active) {
      if (value.spec.widget instanceof RegionWidget) {
        assert.equal(from, target.from);
        assert.equal(to, target.to);
        wrapped = active.some((mark) => mark.spec.attributes?.class === 'sp-region-editing');
      }
    },
  });
  assert.equal(wrapped, true);
});
