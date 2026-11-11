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
/** Canonical preparation proves populated scopes, coordinates and isolated inspection profiles. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { nativeDocumentValueAt } from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import {
  prepareNativeStructure,
  prepareInspectionStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { regionAtPosition } from '../../../../../../frontend/sugarprompt/editor/src/region-index.ts';
import { structureState, publishStructure } from './document-structure-fixture.ts';
import { context } from './document-structure-fixture.ts';

for (const kind of ['prompt', 'wildcard-txt', 'wildcard-csv'] as const)
  test(`${kind} slots share populated actual scopes, groups, queries and coordinate output`, () => {
    const source =
      kind === 'wildcard-csv'
        ? 'h1,h2\r\n"{one}, <lora:A:1.0>","(soft:1.20)"'
        : '{one}, <lora:A:1.0>, (soft:1.20)';
    let { state } = structureState(source, kind);
    const prepared = prepareNativeStructure(state);
    state = publishStructure(state, prepared);
    assert.equal(prepareInspectionStructure(state, { style: 'curly' }), prepared);
    state = state.update({ selection: EditorSelection.single(8, 2) }).state;
    state = state.update({ effects: invalidateCommandContext.of(null) }).state;
    const reused = prepareNativeStructure(state);
    assert.equal(reused, prepared);
    assert.equal(reused.nativeValues, prepared.nativeValues);
    assert.equal(reused.scopes, prepared.scopes);
    assert.equal(reused.loraScopes, prepared.loraScopes);
    assert.equal(reused.pointerTargets, prepared.pointerTargets);
    assert.equal(reused.regions, prepared.regions);
    const scope = reused.scopes.scopes[0]!;
    assert.equal(scope.wildcardGroups.length, 1);
    assert.equal(reused.nativeValues.byId[scope.id]!.scope, scope);
    assert.equal(reused.nativeValues.byId[scope.id]!.semanticMapping!.mapping, scope.mapping);
    assert.equal(reused.scopes.scopes[0]!.tagQueries, prepared.scopes.scopes[0]!.tagQueries);
    assert.equal(
      reused.scopes.scopes[0]!.wildcardGroups,
      prepared.scopes.scopes[0]!.wildcardGroups,
    );
    assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [8, 2]);
    assert.equal(undoDepth(state), 0);
    if (kind !== 'prompt') {
      assert.equal('structure' in prepared, false);
      assert.equal('regionNames' in prepared, false);
      assert.equal('sceneQueries' in prepared, false);
    }
  });

test('TXT physical destinations include padding and empty EOF without joined grammar or scenes', () => {
  for (const ending of [
    '\n',
    '\r',
    '\r\n',
    '\v',
    '\f',
    '\x1c',
    '\x1d',
    '\x1e',
    '\x85',
    '\u2028',
    '\u2029',
  ]) {
    const { state } = structureState(` {one} ${ending}\t${ending}**title${ending}`, 'wildcard-txt');
    const prepared = prepareNativeStructure(state);
    assert.deepEqual(
      prepared.scopes.scopes.map((scope) => scope.mapping.text.length),
      [5, 0, 7, 0],
    );
    assert.equal(prepared.scopes.scopes[0]!.document.wildcardSpans.length, 1);
    assert.equal(
      prepared.scopes.scopes.every(
        (scope) => scope.sceneContexts === null && scope.sceneQueries === null,
      ),
      true,
    );
    assert.equal(
      prepared.nativeValues.entries[3]!.semanticMapping!.logicalRange.from,
      state.doc.length,
    );
    assert.equal(
      nativeDocumentValueAt(state, context(prepared), { from: 0, to: 0 }).status,
      'rejected',
    );
    assert.equal(
      nativeDocumentValueAt(state, context(prepared), {
        from: state.doc.length,
        to: state.doc.length,
      }).status,
      'ready',
    );
  }
});

test('CSV header, malformed storage and neighbor cells cannot contribute grammar or resources', () => {
  const { state } = structureState(
    '"{header}","<lora:Header:1>"\r\n"{one},","{two}, <lora:Actual:1>"\r\n,',
    'wildcard-csv',
  );
  const prepared = prepareNativeStructure(state);
  assert.equal(prepared.scopes.scopes.length, 4);
  assert.deepEqual(
    prepared.scopes.scopes.map((scope) => scope.document.wildcardSpans.length),
    [1, 1, 0, 0],
  );
  assert.equal(prepared.loraScopes.resourceNames.length, 1);
  assert.equal(prepared.loraScopes.resourceNames[0] === 'Actual', true);
  assert.equal(prepared.nativeValues.entries[0]!.scope, null);
  assert.equal(prepared.nativeValues.entries[1]!.scope, null);
  const malformed = prepareNativeStructure(
    structureState('h\r\n"unclosed {one}', 'wildcard-csv').state,
  );
  assert.equal(malformed.documentSemantics.valid, false);
  assert.equal(malformed.scopes.scopes.length, 0);
  assert.equal(malformed.pointerTargets.size, 0);
  assert.equal(malformed.regions.length, 0);
  assert.equal(malformed.loraScopes.resourceNames.length, 0);
});

test('mapped region labels and pointer content preserve doubled quotes, astral scalars and embedded CRLF', () => {
  const { state } = structureState(
    'h\r\n" 🙂, (a""b""c:1.20)\r\n[SEP|R""Q]\r\n{one} "\r\n"[SEP|N]"',
    'wildcard-csv',
  );
  const prepared = prepareNativeStructure(state);
  assert.equal(prepared.regions.length, 2);
  const first = prepared.regions[0]!,
    second = prepared.regions[1]!;
  assert.notEqual(first.scopeId, second.scopeId);
  assert.deepEqual([first.ordinal, second.ordinal], [0, 0]);
  assert.deepEqual([first.from, first.to, first.nameFrom, first.nameTo], [23, 33, 28, 32]);
  assert.deepEqual([second.from, second.to, second.nameFrom, second.nameTo], [43, 50, 48, 49]);
  assert.equal(first.separator.name === 'R"Q', true);
  assert.equal(regionAtPosition(prepared.regions, 29), first);
  assert.equal(regionAtPosition(prepared.regions, 41), null);
  const matches: number[][] = [];
  prepared.pointerTargets.between(11, 11, (from, to, value) => {
    if (value.kind === 'emphasis') {
      assert.equal(value.scopeId, first.scopeId);
      matches.push([from, to, from + value.contentFrom, from + value.contentTo]);
    }
  });
  assert.deepEqual(matches, [[8, 22, 9, 16]]);
  const map = prepared.nativeValues.entries[1]!.semanticMapping!;
  assert.equal(map.decodedToLogical[1], null);
  assert.equal(map.logicalToDecoded[1], null);
});

for (const kind of ['wildcard-txt', 'wildcard-csv'] as const)
  test(`${kind} inspection profiles remain isolated and share the canonical source snapshot`, () => {
    let { state } = structureState(
      kind === 'wildcard-csv' ? 'h\r\n"%%one%%, {two}"' : '%%one%%, {two}',
      kind,
    );
    const native = prepareNativeStructure(state);
    state = publishStructure(state, native);
    const syntax = {
      style: 'custom',
      prefix: '%%',
      suffix: '%%',
      alsoRecognizeCurly: false,
    } as const;
    const inspection = prepareInspectionStructure(state, syntax);
    state = publishStructure(state, inspection, 'inspection');
    assert.equal(inspection.documentSemantics, native.documentSemantics);
    assert.equal(inspection.scopes.values, native.scopes.values);
    assert.notEqual(inspection.scopes, native.scopes);
    assert.equal(inspection.scopes.scopes[0]!.document.wildcardSpans.length, 1);
    assert.equal(inspection.scopes.scopes[0]!.document.wildcardSpans[0]!.outerRange.from, 0);
    assert.equal(prepareInspectionStructure(state, { ...syntax }), inspection);
    assert.equal(prepareNativeStructure(state), native);
    assert.equal(prepareInspectionStructure(state, { style: 'curly' }), native);
    assert.equal(state.field(semanticStructures).inspection, inspection);
  });
