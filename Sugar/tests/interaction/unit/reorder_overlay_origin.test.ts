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
/** Prove a lifted card's overlay is measured from the block that contains it. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { fixedOverlayOrigin } from '../../../frontend/comfyui/ui/interaction/reorder/ReorderOverlayOrigin.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('reads the origin a fixed overlay has inside its host and leaves nothing behind', () => {
  const host = document.createElement('div');
  document.body.append(host);
  // A dialog with a transform places fixed children from its own corner.
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    return (
      this.style.position === 'fixed' && this.parentElement === host
        ? { left: 248, top: 58 }
        : { left: 0, top: 0 }
    ) as DOMRect;
  });
  expect(fixedOverlayOrigin(host)).toEqual({ x: 248, y: 58 });
  expect(host.childElementCount).toBe(0);
});
