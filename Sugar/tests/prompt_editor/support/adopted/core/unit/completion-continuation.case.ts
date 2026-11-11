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
/** Independent conservative syntax specimens qualify the bounded prepared-output continuation gate. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  stableCompletionContent,
  stableTagCompletionPrefix,
  stableCompletionEdit,
  hasCompletionPrefix,
} from '../../../../../../frontend/sugarprompt/core/src/completion/continuation.ts';
import { prepareTagReplacement } from '../../../../../../frontend/sugarprompt/core/src/completion/text.ts';

test('name content permits Unicode, punctuation and model paths while structural characters require fresh semantics', () => {
  for (const kind of ['tag', 'lora'] as const) {
    for (const content of ['', 'hair', 'Straße', '🙂a', 'e\u0301', 'a_b-1.2', '/folder/file'])
      assert.equal(stableCompletionContent(kind, content), true);
    for (const character of [
      '(',
      ')',
      '[',
      ']',
      '{',
      '}',
      '<',
      '>',
      ',',
      ':',
      ' ',
      '\t',
      '\r',
      '\n',
      '\u00a0',
      '\0',
    ])
      assert.equal(
        stableCompletionContent(kind, `a${character}b`),
        (kind === 'lora' && !['<', '>', ':', '\r', '\n', '\0'].includes(character)) ||
          (kind === 'tag' && [' ', '\t'].includes(character)),
      );
  }
  assert.equal(stableCompletionContent('lora', 'folder\\real.pt'), true);
  assert.equal(stableCompletionContent('tag', '\\'), false);
  for (const quote of ["'", '"']) assert.equal(stableCompletionContent('tag', quote), false);
  assert.equal(stableCompletionContent('lora', "folder/o'clock.pt"), true);
});

test('existing word separators and literal escapes permit content refinement while split escapes and short visible prefixes reject', () => {
  for (const prefix of [
    'soft li',
    'soft\tli',
    'real \\(ve',
    'real \\(version\\)',
    '🙂a',
    "o'clock",
    "𐐀'βe",
    "1'2 words",
    "o'",
    "o' cl",
    "a 'bc",
    '"soft li',
  ])
    assert.equal(stableTagCompletionPrefix(prefix), true);
  for (const prefix of ['soft\nli', 'real (ve', 'real\\', '[scene:x', 'a\u00a0b'])
    assert.equal(stableTagCompletionPrefix(prefix), false);
  for (const context of ['\\(', '\\)'])
    assert.equal(stableCompletionEdit('tag', context[0]!, '', 'x', context[1]!), false);
  for (const context of ['ab', '(v', 'r\\', ''])
    assert.equal(stableCompletionEdit('tag', context.slice(0, 1), '', 'x', context.slice(1)), true);
  assert.equal(stableCompletionEdit('lora', '\\', '', 'x', '('), true);
  for (const [before, removed, inserted, after] of [
    ['soft', '', ' ', 'light'],
    ['soft ', '\t', '', 'light'],
    ['"soft ', 'l', 'li', ''],
  ] as const)
    assert.equal(stableCompletionEdit('tag', before, removed, inserted, after), true);
  for (const prefix of ['a', '🙂', '\\(']) assert.equal(hasCompletionPrefix(prefix), false);
  for (const prefix of ['🙂a', '\\(v', '\\(\\)', '🙂🙂', 'e\u0301'])
    assert.equal(hasCompletionPrefix(prefix), true);
});

test('bounded edit seams retain literal Unicode apostrophes and reject changes to their structural meaning', () => {
  for (const [before, removed, inserted, after] of [
    ["o'", 'c', 'k', 'lock'],
    ["o'", 'c', '', 'lock'],
    ['a', 'b', 'c', "'de"],
    ['ab', 'c', '', "'de"],
    ["𐐀'", 'β', '𐐁', 'e'],
    ['', '𐐀', '𐐁', "'β"],
  ] as const)
    assert.equal(stableCompletionEdit('tag', before, removed, inserted, after), true);
  for (const [before, removed, inserted, after] of [
    ["o'", 'c', '', ''],
    ['', 'o', '', "'c"],
    ["o'", 'c', '🙂', ''],
    ['', '𐐀', '🙂', "'β"],
    ['o', '', "'", 'clock'],
    ['o', "'", '', 'clock'],
    ["o'", '', 'c', ''],
    ["o'", '', ' ', 'clock'],
    ["o'", ' ', '', 'clock'],
    ["o'", '', '\t', 'clock'],
  ] as const)
    assert.equal(stableCompletionEdit('tag', before, removed, inserted, after), false);
});

test('prepared right-tail alternatives consume only compatible visible prefixes before the fixed tail', () => {
  const query = {
    prefix: 'soft li',
    replacementRange: { from: 0, to: 7 },
    activeTagEnd: 11,
    fallback: null,
  };
  const prepared = prepareTagReplacement('soft liting', query, 'soft_lighting', true);
  assert.deepEqual(prepared.retainRight, {
    range: { from: 0, to: 7 },
    text: 'soft lighting, ',
  });
  assert.deepEqual(prepared.right!.replacement, {
    range: { from: 0, to: 11 },
    text: 'soft lighting, ',
  });
  for (const prefix of ['soft li', 'soft_ligh', 'soft ligh'])
    assert.equal(prepared.right!.match.consumesSourcePrefix(prefix), true);
  for (const prefix of ['soft light', 'SOFT li', 'unrelated', 'x'.repeat(1_000)])
    assert.equal(prepared.right!.match.consumesSourcePrefix(prefix), false);
  const literal = prepareTagReplacement(
    'real \\(veion\\)',
    {
      ...query,
      prefix: 'real (ve',
      replacementRange: { from: 0, to: 9 },
      activeTagEnd: 14,
    },
    'real_(version)',
    true,
  );
  assert.equal(literal.right!.match.consumesSourcePrefix('real \\(ver'), true);
  assert.equal(literal.right!.match.consumesSourcePrefix('real \\(versi'), false);
});
