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
/** Fixed Unicode whitespace and folding contracts remain stable across repeated normalization and membership calls. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  normalizeTagIdentity,
  prepareTagLexicon,
} from '../../../../../../frontend/sugarprompt/core/src/normalization/lexicon.ts';

test('every declared Python whitespace family collapses while BOM remains literal', () => {
  for (const separator of [
    '\t',
    '\n',
    '\v',
    '\f',
    '\r',
    '\u001c',
    '\u001d',
    '\u001e',
    '\u001f',
    ' ',
    '\u0085',
    '\u00a0',
    '\u1680',
    '\u2000',
    '\u2001',
    '\u2002',
    '\u2003',
    '\u2004',
    '\u2005',
    '\u2006',
    '\u2007',
    '\u2008',
    '\u2009',
    '\u200a',
    '\u2028',
    '\u2029',
    '\u202f',
    '\u205f',
    '\u3000',
  ]) {
    const text = `${separator}Straße${separator}${separator}𐐀_A${separator}`;
    assert.equal(normalizeTagIdentity(text), 'strasse 𐐨 a');
    assert.equal(normalizeTagIdentity(text), 'strasse 𐐨 a');
  }
  assert.equal(normalizeTagIdentity('a\ufeffb'), 'a\ufeffb');
  const lexicon = prepareTagLexicon(['Straße_𐐀', 'a\ufeffb']);
  for (const separator of [' ', '\u001c', '\u2003', '\u3000'])
    assert.equal(lexicon.contains(`STRASSE${separator}𐐨`), true);
  assert.equal(lexicon.contains('ab'), false);
  assert.equal(lexicon.contains('a\ufeffb'), true);
});
