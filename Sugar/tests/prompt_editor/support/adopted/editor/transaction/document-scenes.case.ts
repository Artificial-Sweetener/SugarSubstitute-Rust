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
/** Real native state qualifies source/kind-bound semantic, diagnostic and queue retirement without DOM substitutes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect, EditorState } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import {
  documentDiagnostics,
  installDocumentDiagnostics,
  prepareDocumentDiagnostics,
} from '../../../../../../frontend/sugarprompt/editor/src/document-diagnostics.ts';
import {
  prepareNativeStructure,
  semanticStructures,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  documentMenuTargetAt,
  prepareDocumentMenu,
  currentDocumentMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/document-menu-target.ts';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  sceneQueuePublication,
  sceneQueueTargets,
  prepareSceneQueueTargets,
  sceneQueueTargetAt,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-queue.ts';

/** Publish only already prepared semantic/visual output through actual native effects. */
function prepared(state: EditorState): EditorState {
  const semantic = prepareNativeStructure(state);
  return state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(prepareProjection(state, semantic)),
      installDocumentDiagnostics.of(prepareDocumentDiagnostics(state, semantic.documentSemantics)),
    ],
  }).state;
}
/** Fixture adds one host-configured document kind and the actual diagnostic/queue state owners. */
function fixture(source: string, kind: 'prompt' | 'wildcard-txt' | 'wildcard-csv') {
  const configuration = new Compartment();
  let state = projectionState(source);
  state = state.update({
    effects: StateEffect.appendConfig.of([
      configuration.of(documentKind.of(kind)),
      documentDiagnostics,
      sceneQueuePublication,
      sceneQueueTargets,
    ]),
  }).state;
  return { state: prepared(state), configuration };
}

test('candidate source has no generation scenes or title slots; indexed errors reuse actual findings across reverse selection and read-only inspection', () => {
  const source = '**Header,Prompt\r\n🙂," **Scene, local"',
    marker = source.indexOf('**Scene');
  let { state } = fixture(source, 'wildcard-csv');
  const semantic = state.field(semanticStructures).native!;
  assert.equal(semantic.kind, 'wildcard-csv');
  assert.equal(Object.hasOwn(semantic, 'structure'), false);
  assert.equal(semantic.scopes.scopes[1]!.sceneQueries, null);
  assert.equal(semantic.scopes.scopes[1]!.sceneContexts, null);
  assert.equal(prepareSceneQueueTargets(state), null);
  assert.equal(sceneQueueTargetAt(state, 0), null);
  const target = documentMenuTargetAt(state, marker - 1)!; // One CRLF unit differs before the exact marker.
  assert.equal(target.finding.diagnostic.range.from, marker);
  const menu = prepareDocumentMenu(state, target),
    diagnostics = state.field(documentDiagnostics);
  state = state.update({
    selection: { anchor: state.doc.length, head: marker },
  }).state;
  assert.equal(state.field(documentDiagnostics), diagnostics);
  assert.equal(prepareDocumentDiagnostics(state, semantic.documentSemantics), diagnostics);
  assert.equal(currentDocumentMenu(state, menu), true);
  assert.equal(documentMenuTargetAt(state, target.to), null);
  assert.equal(readExactSource(state) === source, true);
  assert.equal(undoDepth(state), 0);
  state = state.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(documentMenuTargetAt(state, target.from)?.finding, target.finding);
});

test('document-kind replacement retires old semantic/projection/error effects even with identical source; source, directional selection and native history remain intact', () => {
  const { configuration, state: initial } = fixture('shared\r\n**Scene\nlocal', 'prompt');
  let state = prepared(initial.update(exactReplacement(initial, 0, 6, 'changed')).state);
  state = state.update({
    selection: { anchor: state.doc.length, head: 2 },
  }).state;
  const semantic = state.field(semanticStructures).native!,
    projection = state.field(richProjection),
    diagnostics = state.field(documentDiagnostics),
    identity = state.field(editorFreshness).source;
  const source = readExactSource(state),
    depth = undoDepth(state);
  state = state.update({
    effects: configuration.reconfigure(documentKind.of('wildcard-txt')),
  }).state;
  assert.equal(state.field(editorFreshness).source, identity);
  assert.equal(state.field(semanticStructures).native === null, true);
  assert.equal(state.field(richProjection).all.size, 0);
  state = state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(projection),
      installDocumentDiagnostics.of(diagnostics),
    ],
  }).state;
  assert.equal(state.field(semanticStructures).native === null, true);
  assert.equal(state.field(richProjection).all.size, 0);
  assert.equal(state.field(documentDiagnostics).source, null);
  state = prepared(state);
  const candidate = state.field(semanticStructures).native!;
  assert.equal(candidate.kind, 'wildcard-txt');
  assert.equal(Object.hasOwn(candidate, 'structure'), false);
  assert.equal(
    candidate.scopes.scopes.every(
      (scope) => scope.sceneContexts === null && scope.sceneQueries === null,
    ),
    true,
  );
  assert.equal(state.field(documentDiagnostics).findings.size, 1);
  assert.equal(readExactSource(state) === source, true);
  assert.equal(undoDepth(state), depth);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [state.doc.length, 2]);
});

test('source edits retire diagnostics immediately and obsolete exact-source findings cannot publish into new native source', () => {
  let { state } = fixture('  **Scene\r\nbody', 'wildcard-txt');
  const errors = state.field(documentDiagnostics),
    target = documentMenuTargetAt(state, 2)!,
    menu = prepareDocumentMenu(state, target);
  state = state.update(exactReplacement(state, 2, 4, 'stars')).state;
  assert.equal(state.field(documentDiagnostics).findings.size, 0);
  assert.equal(currentDocumentMenu(state, menu), false);
  state = state.update({
    effects: installDocumentDiagnostics.of(errors),
  }).state;
  assert.equal(state.field(documentDiagnostics).findings.size, 0);
  state = prepared(state);
  assert.equal(state.field(documentDiagnostics).findings.size, 0);
});
