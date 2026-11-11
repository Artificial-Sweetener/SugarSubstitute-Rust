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
/** Independent candidate specimens qualify document capability and exact marker ownership. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parseScenes,
  prepareDocumentSemantics,
  validateDocumentKind,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { parseWildcardCsv } from '../../../../../../frontend/sugarprompt/core/src/document/csv.ts';

test('TXT candidate starts use pinned whitespace and physical endings without treating later stars or BOM as a marker', () => {
  for (const ending of ['\n', '\r', '\r\n', '\v', '\f', '\x1c', '\x85', '\u2028', '\u2029']) {
    const source = [
      '🙂 ordinary',
      ' \t**Scene',
      'stars ** later',
      '***',
      '\u00a0**',
      '\ufeff**literal',
    ].join(ending);
    const result = prepareDocumentSemantics(source, 'wildcard-txt');
    assert.equal(result.scenesEnabled, false);
    assert.equal(result.valid, true);
    assert.deepEqual(
      result.unsupportedSceneMarkerRanges,
      ['**Scene', '***', '**'].map((marker, index) => {
        const from = index === 2 ? source.indexOf('\u00a0**') + 1 : source.indexOf(marker);
        return { from, to: from + 2 };
      }),
    );
    assert.equal(Object.isFrozen(result.unsupportedSceneMarkerRanges[0]), true);
  }
});

test('CSV excludes headers and later multiline stars while decoded data starts retain exact quote and Unicode ownership', () => {
  const source =
    '**Header,Prompt\r\n🙂,"  **Scene, ""bright""\r\nforest"\rrow,"prefix\n**later"\n***,plain';
  const result = prepareDocumentSemantics(source, 'wildcard-csv');
  const from = source.indexOf('**Scene'),
    last = source.indexOf('***');
  assert.deepEqual(result.unsupportedSceneMarkerRanges, [
    { from, to: from + 2 },
    { from: last, to: last + 2 },
  ]);
  assert.equal(result.valid, true);
  const csv = parseWildcardCsv(source),
    cell = csv.records[1]![1]!;
  assert.equal(cell.value.includes('"bright"\r\nforest'), true);
  const quote = cell.value.indexOf('"');
  assert.equal(
    source.slice(cell.characterRanges[quote]!.from, cell.characterRanges[quote]!.to),
    '""',
  );
  assert.equal(Object.isFrozen(csv.records[1]), true);
});

test('malformed CSV fails closed, ordinary sources keep scene capability, and disabled scene parsing preserves all exact source', () => {
  for (const source of [
    'Name,Prompt\nfox,"**unclosed',
    'Name,Prompt\nfox,**bad"quote',
    'Name,Prompt\nfox,"**closed"x',
  ]) {
    const result = prepareDocumentSemantics(source, 'wildcard-csv');
    assert.equal(result.valid, false);
    assert.equal(result.unsupportedSceneMarkerRanges.length, 0);
  }
  const source = 'shared\r\n**Scene\rlocal';
  const ordinary = prepareDocumentSemantics(source, 'prompt');
  assert.equal(ordinary.scenesEnabled, true);
  assert.equal(ordinary.unsupportedSceneMarkerRanges.length, 0);
  const disabled = parseScenes(source, false);
  assert.equal(disabled.sourceText === source, true);
  assert.equal(disabled.universalText === source, true);
  assert.equal(disabled.scenes.length, 0);
  assert.throws(() => validateDocumentKind('other'), TypeError);
});
