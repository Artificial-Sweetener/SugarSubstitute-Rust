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
/** Characterize the native tab keyboard and value contract before sharing the presentation primitive. */
import { expect, test } from '@jest/globals';
import { createPhotoProSegments } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProControls.js';

test('native tabs project host selection and emit one user change with roving focus', () => {
  const {
    group,
    segments: [off, on],
    release,
  } = createPhotoProSegments(document);
  group.append(off, on);
  document.body.append(group);
  let changes = 0;
  group.addEventListener('change', () => changes++);
  group.activeid = off.id;
  expect(changes).toBe(0);
  expect(off.tabIndex).toBe(0);
  off.focus();
  off.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(group.activeid).toBe(on.id);
  expect(document.activeElement).toBe(on);
  expect(changes).toBe(1);
  on.click();
  expect(changes).toBe(1);
  off.disabled = true;
  off.click();
  expect(group.activeid).toBe(on.id);
  release();
});
