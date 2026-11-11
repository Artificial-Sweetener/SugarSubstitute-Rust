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
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
/** Literal logical-row expectations independently qualify canonical scene band adaptation and retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment } from '@codemirror/state';
import { projectionState } from '../transaction/projection-state.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  prepareSceneBands,
  sceneBandIntersectsLine,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-line-bands.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';

for (const specimen of [
  {
    source: 'shared\r\n**One\rbody\n\n[SEP] region\nlast\r\n**Two\nend\n',
    expected: [false, true, true, true, true, true, false, false, false],
  },
  {
    source: ' \t\x1c\n**One\nbody\n**Two\n\nlast\n',
    expected: [false, false, false, true, true, true, false],
  },
  { source: '\ufeff\n**One\nbody', expected: [false, true, true] },
  { source: '**One\nbody', expected: [false, false] },
] as const) {
  test('scene groups retain exact logical rows, Python whitespace, CRLF and exclusive EOF ownership', () => {
    const presentation = new Compartment();
    let state = projectionState(specimen.source, presentation);
    const prepared = prepareNativeStructure(state),
      bands = prepareProjection(state, prepared).sceneBands;
    const actual = Array.from({ length: state.doc.lines }, (_, index) => {
      const line = state.doc.line(index + 1);
      return sceneBandIntersectsLine(bands, line.from, line.to);
    });
    assert.deepEqual(actual, specimen.expected);
    assert.ok(Object.isFrozen(bands));
    state = state.update({
      effects: presentation.reconfigure(richRendering.of(false)),
    }).state;
    assert.deepEqual(prepareProjection(state, prepareNativeStructure(state)).sceneBands, []);
    assert.deepEqual(prepareSceneBands(null), []);
  });
}

test('candidate documents and ordinary source have no scene fill authority', () => {
  for (const kind of ['wildcard-txt', 'wildcard-csv'] as const) {
    const state = projectionState('**One\nbody', undefined, kind);
    assert.deepEqual(prepareProjection(state, prepareNativeStructure(state)).sceneBands, []);
  }
  const state = projectionState('ordinary\nsource');
  assert.deepEqual(prepareProjection(state, prepareNativeStructure(state)).sceneBands, []);
});
