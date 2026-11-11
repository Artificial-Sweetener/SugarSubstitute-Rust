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
/** Independent exact-edit contracts preserve literal names, partitions and surrounding authored bytes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  regionNameReplacement,
  validRegionName,
  applyRegionNames,
  changedRegionNames,
  firstRegionNames,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('empty, adjacent and trailing partitions survive minimal literal name edits and all newline spellings', () => {
  const source = '[SEP]\r\n[SEP|Face]\r[SEP]\n';
  const document = parsePrompt(source);
  assert.equal(document.regions.partitions.length, 4);
  assert.equal(
    document.regions.partitions.every(({ sourceRange }) => sourceRange.from === sourceRange.to),
    true,
  );
  for (const [ordinal, name] of [
    [0, '背景 🦊'],
    [1, ' <literal>|[ '],
    [1, ''],
    [2, '  '],
  ] as const) {
    const edit = regionNameReplacement(document, ordinal, name);
    const observed = source.slice(0, edit.from) + edit.text + source.slice(edit.to);
    const renamed = parsePrompt(observed);
    assert.equal(renamed.regions.separators[ordinal]!.name === (name || null), true);
    assert.equal(renamed.regions.partitions.length, 4);
    assert.equal(
      renamed.regions.partitions.every(({ sourceRange }) => sourceRange.from === sourceRange.to),
      true,
    );
    assert.equal(observed.match(/\r\n|\r|\n/gu)?.join('') === '\r\n\r\n', true);
    assert.deepEqual(
      [edit.from, edit.to],
      name && ordinal === 1
        ? [12, 16]
        : ordinal === 0
          ? [0, 5]
          : ordinal === 1
            ? [7, 17]
            : [18, 23],
    );
  }
});

test('shared identities preserve first authored ordinals and apply original-source edits once without creating separators', () => {
  const first = parsePrompt('[SEP]\r\n[SEP| Later ]\n'),
    second = parsePrompt('[SEP| First ]\rbody\n[SEP|Other]\n');
  assert.equal(
    firstRegionNames([first, second])
      .map(({ ordinal, name }) => `${ordinal}:${name}`)
      .join() === '1:Later,0:First',
    true,
  );
  assert.equal(changedRegionNames(first, parsePrompt('[SEP]\n')).length, 0);
  const result = applyRegionNames(second, [
    { ordinal: 1, name: '背景 🦊' },
    { ordinal: 0, name: '' },
    { ordinal: 9, name: 'ignored' },
  ]);
  assert.equal(result.source === '[SEP]\rbody\n[SEP|背景 🦊]\n', true);
  assert.equal(result.replacements.length, 2);
  assert.equal(result.replacements[0]!.from < result.replacements[1]!.from, true);
  assert.throws(() => applyRegionNames(first, [{ ordinal: -1, name: 'x' }]), RangeError);
  assert.throws(
    () =>
      applyRegionNames(first, [
        { ordinal: 0, name: 'x' },
        { ordinal: 0, name: 'y' },
      ]),
    RangeError,
  );
  assert.throws(() => applyRegionNames(first, [{ ordinal: 0, name: ']' }]), TypeError);
});

test('separator terminators and unavailable ordinals reject explicitly; literal Unicode, pipes and whitespace stay valid', () => {
  const document = parsePrompt('[SEP|Face]');
  for (const name of [']', 'a\r', 'a\n']) {
    assert.equal(validRegionName(name), false);
    assert.throws(() => regionNameReplacement(document, 0, name), TypeError);
  }
  for (const ordinal of [-1, 0.5, 1, NaN, Infinity])
    assert.throws(() => regionNameReplacement(document, ordinal, 'safe'), RangeError);
  for (const name of ['', '[', '|', ' 🦊 ', '背景']) assert.equal(validRegionName(name), true);
});
