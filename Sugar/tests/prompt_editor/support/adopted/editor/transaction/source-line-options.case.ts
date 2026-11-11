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
/** Independent public visual-intent rejection specimens require no browser or semantic oracle. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { validateSourceLineChrome } from '../../../../../../frontend/sugarprompt/editor/src/source-line-contracts.ts';

test('source-line configuration accepts finite CSS insets and rejects malformed host intent', () => {
  for (const contentLeftInset of [0, 0.5, 28, 100_000])
    assert.doesNotThrow(() => validateSourceLineChrome({ enabled: false, contentLeftInset }));
  for (const contentLeftInset of [-1, NaN, Infinity, 100_001])
    assert.throws(() => validateSourceLineChrome({ enabled: true, contentLeftInset }), RangeError);
  assert.throws(
    () =>
      validateSourceLineChrome({
        enabled: 'yes' as unknown as boolean,
        contentLeftInset: 0,
      }),
    RangeError,
  );
});
