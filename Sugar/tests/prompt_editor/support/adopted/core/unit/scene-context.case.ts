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
/** Independent scene boundaries, immutable reuse and exact materialization contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  materializeScenePrompt,
  parseScenes,
  prepareSceneContexts,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('scene materialization trims only boundary whitespace and preserves interior authored source', () => {
  assert.equal(
    materializeScenePrompt(' \u0085shared\r\ninside\x1c', '\tlocal\rbody\n'),
    'shared\r\ninside\n\nlocal\rbody',
  );
  assert.equal(materializeScenePrompt(' \t', ' local '), 'local');
  assert.equal(materializeScenePrompt(' shared ', '\n'), 'shared');
  assert.equal(materializeScenePrompt('\u3000', '\x1f'), '');
  assert.equal(materializeScenePrompt('\ufeff', '\ufeff'), '\ufeff\n\n\ufeff');
});

test('source ownership includes heading indentation, the next heading boundary and final EOF without rewriting titles', () => {
  const source = '🙂 shared\r\n  ** Straße\r\none\r**STRASSE\ntwo';
  const document = parseScenes(source),
    prepared = prepareSceneContexts(document);
  const [first, second] = prepared.scenes;
  assert.equal(prepared.document, document);
  assert.equal(first?.scene, document.scenes[0]);
  assert.equal(first?.scene?.marker.title, 'Straße');
  assert.equal(second?.scene?.marker.duplicate, true);
  assert.equal(second?.scene?.marker.normalizedKey, 'strasse');
  assert.equal(prepared.at(source.indexOf('  **')), first);
  assert.equal(prepared.at(source.indexOf('**STRASSE') - 1), first);
  assert.equal(prepared.at(source.indexOf('**STRASSE')), second);
  assert.equal(prepared.at(source.length), second);
  assert.equal(prepared.at(source.length + 10), second);
  assert.equal(prepared.at(-5), prepared.universal);
  assert.equal(first?.effectivePromptText, '🙂 shared\n\none');
  assert.equal(second?.effectivePromptText, '🙂 shared\n\ntwo');
  assert.equal(prepared.universal.effectivePromptText, '🙂 shared\r\n');
  for (const value of [0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])
    assert.throws(() => prepared.at(value), RangeError);
  assert.equal(Object.isFrozen(prepared), true);
  assert.equal(Object.isFrozen(prepared.scenes), true);
  assert.equal(Object.isFrozen(first?.sourceRange), true);
});

test('effective text enumeration includes every parsed scene across physical and Unicode hard lines and deduplicates real output', () => {
  for (const ending of [
    '\r',
    '\n',
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
    const source = ['shared', '**First', 'one', '**Second', 'two', '**Third', 'one'].join(ending);
    const prepared = prepareSceneContexts(parseScenes(source));
    assert.deepEqual(prepared.effectivePromptTexts, [
      `shared${ending}`,
      'shared\n\none',
      'shared\n\ntwo',
    ]);
    assert.equal(prepared.at(source.indexOf('two')), prepared.scenes[1]);
  }
  assert.deepEqual(
    prepareSceneContexts(parseScenes('**First\ntext\n**Second\ntext')).effectivePromptTexts,
    ['text'],
  );
  const plain = ' exact\r\nsource ';
  const prepared = prepareSceneContexts(parseScenes(plain));
  assert.deepEqual(prepared.effectivePromptTexts, [plain]);
  assert.equal(prepared.at(plain.length), prepared.universal);
  assert.deepEqual(prepareSceneContexts(parseScenes('')).effectivePromptTexts, ['']);
});

test('large prepared scene lookup returns the same actual contexts across directional caret movement', () => {
  const source = Array.from(
    { length: 4096 },
    (_, index) => `**Title ${index}\nbody ${index}\n`,
  ).join('');
  const universal = `${' '.repeat(250_000)}shared\r\n`;
  const prepared = prepareSceneContexts(parseScenes(universal + source));
  assert.equal(prepared.effectivePromptTexts.length, 4097);
  assert.equal(prepared.scenes[4095]?.effectivePromptText === 'shared\n\nbody 4095', true);
  for (const index of [4095, 0, 2048, 77, 4095]) {
    const context = prepared.scenes[index]!;
    assert.equal(prepared.at(context.sourceRange.from), context);
    assert.equal(prepared.at(context.sourceRange.to - 1), context);
  }
});
