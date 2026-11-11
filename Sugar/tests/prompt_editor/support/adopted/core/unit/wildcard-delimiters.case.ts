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
/** Independent delimiter specimens define pinned Unicode classification, scalar limits and exact wrapper ownership. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  serializePrompt,
  type WildcardSyntax,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Custom profiles are parser APIs; the native application's active syntax remains curly. */
function custom(
  prefix: string,
  suffix: string,
  alsoRecognizeCurly = false,
): Extract<WildcardSyntax, { style: 'custom' }> {
  return { style: 'custom', prefix, suffix, alsoRecognizeCurly };
}

test('custom delimiter validation uses pinned Unicode 15 rather than newer JavaScript letter assignments', () => {
  for (const part of [
    '\u1c89',
    '\u{2ebf0}',
    '\ud800',
    '\u0301',
    '\ufeff',
    '🙂'.repeat(8),
    'abcdefg!',
  ]) {
    for (const profile of [custom(part, '!!'), custom('!!', part)]) {
      const source = `${profile.prefix}animals${profile.suffix}`;
      const document = parsePrompt(source, profile);
      assert.equal(document.wildcardSpans.length, 1);
      assert.equal(document.wildcardSpans[0]!.identifier === 'animals', true);
      assert.equal(serializePrompt(document) === source, true);
    }
  }
  for (const part of [
    'a',
    '日',
    '𐐀',
    '\u{1e4d0}',
    '²',
    'Ⅳ',
    '½',
    '',
    '🙂'.repeat(9),
    'abcdefgh!',
    '\x00',
    '\x1f',
    '\x1c',
    '\u0085',
    '\u00a0',
    ' ',
    '\t',
    '\n',
    '\r',
    "'",
    '"',
    '/',
    '\\',
    '{',
    '}',
  ]) {
    for (const profile of [custom(part, '!!'), custom('!!', part)])
      assert.throws(() => parsePrompt('', profile), RangeError);
  }
  for (const profile of [custom('<', '>'), custom('(', ')')])
    assert.throws(() => parsePrompt('', profile), RangeError);
});

test('wrapper APIs preserve scalar ranges, tag ownership, curly compatibility and configured priority', () => {
  const prefix = '🙂'.repeat(8),
    suffix = '!!';
  const source = `head, ${prefix}csv:characters:name|2${suffix}, {animals}`;
  const document = parsePrompt(source, custom(prefix, suffix));
  const span = document.wildcardSpans[0]!;
  assert.equal(document.wildcardSpans.length, 1);
  assert.deepEqual(span.outerRange, { from: 6, to: 45 });
  assert.deepEqual(span.contentRange, { from: 22, to: 43 });
  assert.equal(
    span.identifier === 'characters' && span.csvColumn === 'name' && span.tag === '2',
    true,
  );
  assert.equal(parsePrompt(source, custom(prefix, suffix, true)).wildcardSpans.length, 2);
  assert.equal(
    parsePrompt('__animals__, {animals}', { style: 'double-underscore' }).wildcardSpans.length,
    2,
  );
  assert.equal(serializePrompt(document) === source, true);
});
