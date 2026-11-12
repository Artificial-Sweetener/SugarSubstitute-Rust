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
/** Independent caret and commit specimens define editable-token diagnostic visibility. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  diagnosticIsActive,
  isDiagnosticCommitBoundary,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('diagnostic visibility excludes the opener, hides interiors and requires a committed end', () => {
  for (const committed of [false, true]) {
    const active = Array.from({ length: 9 }, (_, caret) =>
      diagnosticIsActive(2, 6, caret, committed),
    );
    assert.deepEqual(active, [false, false, false, true, true, true, !committed, false, false]);
  }
  assert.equal(diagnosticIsActive(0, 0, 0, false), true);
  for (const args of [
    [-1, 3, 0],
    [3, 2, 0],
    [0, 3, -1],
    [0, 3, 1.5],
    [0, Number.NaN, 1],
  ] as const)
    assert.throws(() => diagnosticIsActive(args[0], args[1], args[2], false), RangeError);
});

test('commit characters use pinned whitespace and exact punctuation without accepting partial strings', () => {
  for (const character of Array.from(',;:.!?)]}"\' \t\r\n\u0085\u001c\u00a0'))
    assert.equal(isDiagnosticCommitBoundary(character), true);
  for (const character of ['', 'a', '_', '-', '(', '[', '{', '🙂', '\ufeff', 'a '])
    assert.equal(isDiagnosticCommitBoundary(character), false);
});
