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
/** Independent correction specimens qualify exact physical-line and scalar boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareTriggerInsertionSlot,
  triggerInsertionText,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('trigger insertion preserves CR/LF/CRLF and never imports a separator from the previous line', () => {
  for (const newline of ['\r', '\n', '\r\n']) {
    const source = `😀 first${newline}  ${newline}last`,
      start = '😀 first'.length;
    const first = prepareTriggerInsertionSlot(source, {
      kind: 'position',
      position: 3,
    });
    assert.equal(first?.range.from, start);
    assert.equal(
      first && triggerInsertionText(first, '  red_hair, (ears)  '),
      ', red_hair, (ears)',
    );
    const empty = prepareTriggerInsertionSlot(source, {
      kind: 'position',
      position: start + newline.length,
    });
    assert.equal(empty?.range.from, start + newline.length + 2);
    assert.equal(empty && triggerInsertionText(empty, 'word'), 'word');
  }
});

test('trigger insertion refuses reversed, CRLF-interior, surrogate-interior and invalid positions', () => {
  const source = '😀\r\ntext';
  for (const position of [-1, 1, 3, 9, 1.5, NaN])
    assert.equal(prepareTriggerInsertionSlot(source, { kind: 'position', position }), null);
  for (const range of [
    { from: 4, to: 2 },
    { from: 2, to: 3 },
    { from: 1, to: 4 },
    { from: 4, to: 4 },
  ])
    assert.equal(prepareTriggerInsertionSlot(source, { kind: 'selection', range }), null);
  const slot = prepareTriggerInsertionSlot(source, {
    kind: 'selection',
    range: { from: 4, to: 8 },
  });
  assert.equal(slot && triggerInsertionText(slot, '\u0085'), null);
  assert.equal(slot && triggerInsertionText(slot, '\u0085a_b, \\(c\\)\u0085'), 'a_b, \\(c\\)');
});
