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
/** Explicit public inspection qualifies actual canonical data and native state preservation without DOM claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { structureState } from './document-structure-fixture.ts';
import { inspectStructure } from '../../../../../../frontend/sugarprompt/editor/src/semantic-inspection.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  lineEndings,
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** An unmounted nominal view delegates all publications to actual CodeMirror transactions. */
function inspectionFixture(source: string, kind: PromptDocumentKind) {
  let state = structureState(source, kind).state;
  let publications = 0;
  // Exact external boundary: no browser, input surface or hidden editor is constructed.
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('inspection-fixture-view');
  Object.defineProperties(boundary, {
    state: { get: () => state },
    compositionStarted: { value: false, writable: true },
    dispatch: {
      value: (spec: TransactionSpec) => {
        const transaction = state.update(spec);
        assert.equal(transaction.annotation(Transaction.addToHistory), false);
        state = transaction.state;
        publications++;
      },
    },
  });
  return {
    view: boundary,
    state: () => state,
    publications: () => publications,
    /** Independent source changes retire inspection through the actual native field. */
    replace(text: string) {
      state = state.update(exactReplacement(state, 0, readExactSource(state).length, text)).state;
    },
  };
}

for (const [kind, source, values, tokenCounts] of [
  ['prompt', '(cat:1.20), <lora:m:0.7>', 1, [1, 1]],
  ['wildcard-txt', ' (cat:1.20) \r\n<lora:m:0.7>\n', 3, [1, 1]],
  ['wildcard-csv', 'head,skip\r\n"(cat:1.20)",<lora:m:0.7>', 2, [1, 1]],
] as const) {
  test(`${kind} public inspection shares populated canonical scopes and retains native state`, () => {
    const f = inspectionFixture(source, kind);
    const before = f.state();
    const result = inspectStructure(f.view, { style: 'curly' });
    assert.equal(result.status, 'ready');
    assert.equal(result.kind, kind);
    const native = f.state();
    const actual = native.field(semanticStructures).inspection!;
    assert.equal(result.scopes, actual.scopes);
    assert.equal(result.documentSemantics, actual.documentSemantics);
    assert.equal(result.loraResourceNames, actual.loraScopes.resourceNames);
    assert.equal(result.scopes.scopes.length, values);
    assert.deepEqual(
      result.scopes.scopes.reduce(
        (counts, scope) => [
          counts[0]! + scope.document.emphasisSpans.length,
          counts[1]! + scope.document.loraSpans.length,
        ],
        [0, 0],
      ),
      tokenCounts,
    );
    assert.equal(Object.hasOwn(result, 'structure'), kind === 'prompt');
    if (result.kind === 'prompt') {
      assert.equal(actual.kind, 'prompt');
      assert.equal(result.structure, actual.structure);
    } else {
      for (const scope of result.scopes.scopes) {
        assert.equal(scope.sceneContexts, null);
        assert.equal(scope.sceneQueries, null);
        assert.equal(scope.container.editability, 'editable');
      }
    }
    assert.equal(readExactSource(native) === source, true);
    assert.equal(native.doc, before.doc);
    assert.equal(native.field(lineEndings), before.field(lineEndings));
    assert.equal(native.selection, before.selection);
    assert.deepEqual([undoDepth(native), redoDepth(native)], [0, 0]);
    const again = inspectStructure(f.view, { style: 'curly' });
    assert.equal(again.status, 'ready');
    assert.equal(again.scopes, result.scopes);
    assert.equal(again.documentSemantics, result.documentSemantics);
    assert.equal(again.loraResourceNames, result.loraResourceNames);
    assert.equal(f.publications(), 1);
  });
}

test('CSV inspection maps doubled quotes and scalar boundaries, excludes header grammar, and retires on source replacement', () => {
  const f = inspectionFixture('"(header:2.00)"\r\n"🙂,""quoted"", (cat:1.20)"', 'wildcard-csv');
  const result = inspectStructure(f.view, { style: 'curly' });
  assert.equal(result.status, 'ready');
  assert.equal(result.kind, 'wildcard-csv');
  const scope = result.scopes.scopes[0]!;
  assert.equal(scope.mapping.text === '🙂,"quoted", (cat:1.20)', true);
  assert.equal(scope.mapping.decodedToSource[0], 18);
  assert.equal(scope.mapping.decodedToSource[1], null);
  assert.equal(scope.mapping.decodedToSource[2], 20);
  assert.equal(scope.document.emphasisSpans.length, 1);
  assert.deepEqual(result.loraResourceNames, []);
  f.replace('head\n"<lora:n:0.5>"');
  const changed = inspectStructure(f.view, { style: 'curly' });
  assert.equal(changed.status, 'ready');
  assert.notEqual(changed.scopes, result.scopes);
  assert.deepEqual(changed.loraResourceNames, ['n']);
  assert.equal(changed.scopes.scopes[0]!.document.loraSpans.length, 1);
  assert.deepEqual([undoDepth(f.state()), redoDepth(f.state())], [1, 0]);
});

test('composition defers public structure inspection and cannot publish a prepared slot', () => {
  const f = inspectionFixture('(cat:1.20)', 'prompt');
  assert.equal(Reflect.set(f.view, 'compositionStarted', true), true);
  const before = f.state();
  assert.deepEqual(inspectStructure(f.view, { style: 'curly' }), {
    status: 'composing',
  });
  assert.equal(f.state(), before);
  assert.equal(f.publications(), 0);
});
