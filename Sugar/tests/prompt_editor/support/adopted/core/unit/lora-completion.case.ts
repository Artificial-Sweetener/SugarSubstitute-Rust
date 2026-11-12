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
/** Independent scheduler and name-slot contracts preserve exact source, Unicode offsets and every valid schedule field. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareLoraQueries,
  loraReplacement,
  loraScheduleText,
  loraScheduleWeight,
  loraCompletionSuffix,
  parsePrompt,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('complete name-field bounds contain right-side filename text while typed prefixes still end at the caret', () => {
  for (const [source, caret, end] of [
    ['<lora:reaZ', 9, 10],
    ['<lora:reaZ tail', 9, 10],
    ['<lora:reaZ>', 9, 10],
    ['<lora:rea🙂 (draft): +0.80:0.60:IN=0.2>', 9, 19],
    ['<lora:reaZ:bad>', 9, 10],
    ['<lora:reaZ\r\nnext', 9, 10],
  ] as const) {
    const query = prepareLoraQueries(source).queryAt(caret, false)!;
    assert.deepEqual(query.nameRange, { from: 6, to: caret });
    assert.deepEqual(query.nameFieldRange, { from: 6, to: end });
    assert.equal(Object.isFrozen(query.nameFieldRange), true);
  }
});

test('LoRA marker and field preparation uses exact source boundaries across Unicode expansions, astral scalars and CRLF', () => {
  for (const prefix of ['', 'ß ', 'İ ﬃ 🙂\r\n']) {
    const source = prefix + '<LoRA:mod: +０.８０:0.60:IN=0.2> tail';
    const caret = prefix.length + 9;
    const query = prepareLoraQueries(source).queryAt(caret, false)!;
    assert.deepEqual(
      [
        query.queryText,
        query.nameRange.from,
        query.nameRange.to,
        query.replacementRange.from,
        query.replacementRange.to,
        query.typedWeightText,
        query.preservedSuffix,
        query.hasClosingBracket,
      ],
      [
        'mod',
        prefix.length + 6,
        caret,
        prefix.length,
        source.indexOf('>') + 1,
        '+０.８０',
        ': +０.８０:0.60:IN=0.2',
        true,
      ],
    );
    assert.equal(
      loraReplacement(source, query, 'folder\\chosen').text ===
        '<lora:folder\\chosen: +０.８０:0.60:IN=0.2>',
      true,
    );
  }
});

test('closed and incomplete name slots exclude weights, closed tails and active selections without consuming a later token', () => {
  for (const source of [
    '<lora:',
    '<lora:mod',
    '<lora:mod\n<lora:other:1.00>',
    '<lora:mod:0.8>',
    '<lora:mod:bad>',
  ]) {
    const caret = source.startsWith('<lora:mod') ? 9 : 6;
    const prepared = prepareLoraQueries(source),
      query = prepared.queryAt(caret, false)!;
    assert.equal(query.queryText === source.slice(6, caret), true);
    assert.equal(prepared.queryAt(caret, true), null);
    assert.equal(
      loraReplacement(source, query, 'chosen').text ===
        (source.includes(':0.8>') ? '<lora:chosen:0.8>' : '<lora:chosen:1.00>'),
      true,
    );
    if (source.includes('\n')) assert.equal(query.replacementRange.to, caret);
    if (query.hasClosingBracket) {
      assert.equal(prepared.queryAt(source.length, false), null);
      assert.equal(prepared.queryAt(source.indexOf(':', 6) + 1, false), null);
    }
  }
});

test('schedule builder retains valid numeric spelling and applies the exact default policy to absent or malformed weights', () => {
  for (const weight of [null, undefined, '', '  ', 'no', '1e2', '1:2', 'NaN'])
    assert.equal(loraScheduleWeight(weight), '1.00');
  for (const weight of ['+1.20', '-0', '.8', '1.', '０.８０', '1'.repeat(200)])
    assert.equal(loraScheduleWeight(' \t' + weight + '\u2003'), weight);
  assert.equal(loraScheduleText('folder\\model', '-.5') === '<lora:folder\\model:-.5>', true);
  for (const name of ['', ' ', 'x:1', 'x>tail', '<lora:x', 'x\r\ny', 'x\u0000'])
    assert.throws(() => loraScheduleText(name), TypeError);
});

test('friendly LoRA ghost matches scalar case/separator equivalence and excludes unrelated substring matches', () => {
  assert.equal(loraCompletionSuffix('mid', 'Midna', 'folder\\raw'), 'na');
  assert.equal(loraCompletionSuffix('', 'Midna', 'folder\\raw'), 'Midna');
  assert.equal(loraCompletionSuffix('folder/raw_', 'Other', 'folder\\raw name'), 'name');
  assert.equal(loraCompletionSuffix('dna', 'Midna', 'folder\\raw'), '');
  assert.equal(loraCompletionSuffix('STRASS', 'Straße', 'Straße'), '');
});

test("name completion preserves the parser's first/second/block fields and rejects noneditable source boundaries", () => {
  const source = '🙂\r\n<lora:model:0.80:0.60:IN=0.2,OUT=0.4>';
  const query = prepareLoraQueries(source).queryAt(15, false)!;
  const edit = loraReplacement(source, query, 'chosen');
  const changed = source.slice(0, edit.range.from) + edit.text + source.slice(edit.range.to);
  const before = parsePrompt(source).loraSpans[0]!,
    after = parsePrompt(changed).loraSpans[0]!;
  assert.deepEqual(
    [
      after.firstWeight,
      after.secondWeight,
      changed.slice(after.blockWeightsRange!.from, after.blockWeightsRange!.to),
    ],
    [
      before.firstWeight,
      before.secondWeight,
      source.slice(before.blockWeightsRange!.from, before.blockWeightsRange!.to),
    ],
  );
  for (const boundary of [-1, 1, 3, 0.5, source.length + 1])
    assert.throws(() => prepareLoraQueries(source).queryAt(boundary, false), RangeError);
  assert.throws(() => loraReplacement(source + ' ', query, 'chosen'), TypeError);
  assert.throws(
    () => loraReplacement(source, { ...query, preservedSuffix: ':1>tail' }, 'chosen'),
    TypeError,
  );
});
