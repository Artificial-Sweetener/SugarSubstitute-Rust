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
/** Prove a refused menu clipboard command is said aloud, and nothing else is. */
import { expect, test } from '@jest/globals';
import { clipboardRefusalObserver } from '../../../frontend/comfyui/ui/prompt_editor/runtime/ClipboardRefusal.js';

test('a denied or unavailable clipboard is reported to the person once per command', () => {
  let refusals = 0;
  const failures: unknown[] = [];
  const observer = clipboardRefusalObserver(
    () => (refusals += 1),
    (cause) => failures.push(cause),
  );
  observer.observe({ status: 'denied' });
  observer.observe({ status: 'unavailable' });
  expect(refusals).toBe(2);
  for (const status of ['completed', 'empty', 'cancelled', 'read-only', 'stale'] as const)
    observer.observe({ status });
  expect(refusals).toBe(2);
  const cause = new Error('unexpected');
  observer.error(cause);
  expect(failures).toEqual([cause]);
});
