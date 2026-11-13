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
/** Native range-index conversion and actual prepared-output reuse, independently of pointer geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text } from '@codemirror/state';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactSourceExtension,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

test('pointer ranges convert mixed source endings to native UTF-16 and reuse the actual index across selection', () => {
  const source = '🙂,\r\n (soft light:1.20), tail';
  let state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), editorFreshness, semanticStructures],
  });
  const prepared = prepareNativeStructure(state);
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepared,
    }),
  }).state;
  const matches: {
    kind: string;
    from: number;
    to: number;
    contentFrom: number;
    contentTo: number;
  }[] = [];
  prepared.pointerTargets.between(12, 12, (from, to, value) => {
    matches.push({
      kind: value.kind,
      from,
      to,
      contentFrom: from + value.contentFrom,
      contentTo: from + value.contentTo,
    });
  });
  assert.deepEqual(matches, [
    { kind: 'segment', from: 5, to: 22, contentFrom: 5, contentTo: 22 },
    { kind: 'emphasis', from: 5, to: 22, contentFrom: 6, contentTo: 16 },
  ]);
  state = state.update({ selection: { anchor: 6, head: 16 } }).state;
  assert.equal(prepareNativeStructure(state).pointerTargets, prepared.pointerTargets);
  state = state.update(exactReplacement(state, 0, 2, 'a')).state;
  assert.equal(state.field(semanticStructures).native, null);
  assert.notEqual(prepareNativeStructure(state).pointerTargets, prepared.pointerTargets);
});
