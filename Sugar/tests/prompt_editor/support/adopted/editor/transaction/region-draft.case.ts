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
/** Actual CodeMirror decoration bounds conserve one scoped regional draft and its finite native lifetime. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type EditorState } from '@codemirror/state';
import { regionDraftDecorationSets } from './region-draft-observation.ts';
import { undoDepth } from '@codemirror/commands';
import { weightStateFor } from './weight-command-state.ts';
import {
  regionDraft,
  setRegionDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/region-draft.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Observe the actual state-owned decoration facet; this fixture has no viewport callback or mounted view. */
function draftBounds(state: EditorState): readonly { from: number; to: number }[] {
  const bounds: { from: number; to: number }[] = [];
  for (const provider of regionDraftDecorationSets(state)) {
    provider.between(0, state.doc.length, (from, to, decoration) => {
      if (decoration.spec.attributes?.class === 'sp-region-editing') bounds.push({ from, to });
    });
  }
  return bounds;
}

test('two quoted CSV regions with local ordinal zero paint only the actual scope target, conserving header, sibling, multiline Unicode and native history', () => {
  const source = '"[SEP|Header]",h\r\n"[SEP|A]","[SEP|B]\r\n🙂 body"\rtail,last\n';
  let state = weightStateFor(source, 0, 0, 'wildcard-csv');
  state = state.update({
    effects: StateEffect.appendConfig.of(regionDraft),
  }).state;
  const native = state.field(semanticStructures).native!,
    first = native.regions[0]!,
    second = native.regions[1]!;
  assert.equal(native.regions.length, 2);
  assert.equal(first.ordinal, 0);
  assert.equal(second.ordinal, 0);
  assert.notEqual(first.scopeId, second.scopeId);
  assert.equal(state.doc.lineAt(first.from).from, state.doc.lineAt(second.from).from);
  assert.equal(
    native.nativeValues.byId[second.scopeId]!.scope!.document.sourceText.includes('🙂'),
    true,
  );
  for (const target of [first, second]) {
    state = state.update({
      effects: setRegionDraft.of({
        freshness: state.field(editorFreshness),
        scopeId: target.scopeId,
        ordinal: target.ordinal,
        width: 33,
      }),
    }).state;
    assert.deepEqual(draftBounds(state), [{ from: target.from, to: target.to }]);
    const sibling = target === first ? second : first;
    assert.equal(
      draftBounds(state).every((range) => range.to <= sibling.from || sibling.to <= range.from),
      true,
    );
    assert.equal(
      draftBounds(state).every((range) => range.from > state.doc.line(1).to),
      true,
    );
    assert.equal(readExactSource(state) === source, true);
    assert.equal(undoDepth(state), 0);
    assert.equal(state.field(semanticStructures).native, native);
  }
  const draft = state.field(regionDraft)!;
  state = state.update({
    selection: { anchor: state.doc.length, head: second.from },
  }).state;
  assert.equal(state.field(regionDraft), draft);
  assert.deepEqual(draftBounds(state), [{ from: second.from, to: second.to }]);
  state = state.update(exactReplacement(state, source.length, source.length, 'x')).state;
  assert.equal(state.field(regionDraft), null);
  assert.deepEqual(draftBounds(state), []);
  state = state.update({ effects: setRegionDraft.of(draft) }).state;
  assert.equal(state.field(regionDraft), null);
  assert.deepEqual(draftBounds(state), []);
});

test('ordinary prompt draft preserves its existing full-row line decoration and source/history contract', () => {
  const source = '🙂\r\n[SEP|Face]\r\nbody';
  let state = weightStateFor(source);
  state = state.update({
    effects: StateEffect.appendConfig.of(regionDraft),
  }).state;
  const target = state.field(semanticStructures).native!.regions[0]!;
  state = state.update({
    effects: setRegionDraft.of({
      freshness: state.field(editorFreshness),
      scopeId: target.scopeId,
      ordinal: target.ordinal,
      width: 42,
    }),
  }).state;
  const from = state.doc.lineAt(target.from).from;
  assert.deepEqual(draftBounds(state), [{ from, to: from }]);
  assert.equal(readExactSource(state) === source, true);
  assert.equal(undoDepth(state), 0);
});
