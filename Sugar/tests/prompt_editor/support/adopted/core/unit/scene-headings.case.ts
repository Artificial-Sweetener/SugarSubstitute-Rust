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
/** Independent heading projection preserves visible whitespace, literal title spelling and error identity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parseScenes,
  prepareSceneHeadings,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('heading prefixes conceal indentation/markers while literal titles and trailing whitespace stay source-backed', () => {
  const source = 'shared\r\n \t**  Straße   Interior \t\r\nbody\r**<img src=x>\nlast';
  const document = parseScenes(source),
    headings = prepareSceneHeadings(document);
  assert.equal(headings[0]?.marker, document.scenes[0]?.marker);
  assert.equal(
    source.slice(headings[0]!.concealedRange.from, headings[0]!.concealedRange.to),
    ' \t**  ',
  );
  assert.equal(
    source.slice(headings[0]!.visibleRange.from, headings[0]!.visibleRange.to),
    'Straße   Interior \t',
  );
  assert.equal(
    source.slice(headings[1]!.visibleRange.from, headings[1]!.visibleRange.to),
    '<img src=x>',
  );
  assert.equal(
    headings.every((heading) => Object.isFrozen(heading) && Object.isFrozen(heading.visibleRange)),
    true,
  );
});

test('duplicate title errors belong to subsequent markers and host keys classify exact normalized identities', () => {
  const source = '** Straße\none\n**STRASSE\ntwo\n**Cafe   Interior\nthree';
  const document = parseScenes(source);
  const headings = prepareSceneHeadings(document, new Set(['cafe interior']));
  assert.deepEqual(
    headings.map((heading) => [heading.error, heading.hostError]),
    [
      [false, false],
      [true, false],
      [true, true],
    ],
  );
  assert.deepEqual(prepareSceneHeadings(parseScenes('**  \nplain')), []);
});
