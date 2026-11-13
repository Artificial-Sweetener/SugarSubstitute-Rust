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
/** Preserve the public library entrypoint through the current Cube cart. */
import { expect, jest, test } from '@jest/globals';
import { createPublicApi } from '../../../frontend/comfyui/ui/index.js';
import { SugarCubesUI } from '../../../frontend/comfyui/ui/SugarCubesUI.js';

test('delegates openLibrary to the supplied cart without constructing an old browser', () => {
  const ui = new SugarCubesUI();
  const openCart = jest.fn();
  const api = createPublicApi(ui, { openLibrary: openCart });
  api.openLibrary();
  expect(openCart).toHaveBeenCalledTimes(1);
  expect(Object.hasOwn(ui, 'cubeBrowser')).toBe(false);
  expect(Object.hasOwn(ui.overlayManager, 'placement')).toBe(false);
  expect(document.querySelector('.sugarcubes-browser')).toBeNull();
  ui.dispose();
});

test('desktop configuration and read-only receipt delegate to the same presentation owner', () => {
  const ui = new SugarCubesUI();
  const receipt = {
    schemaVersion: 1 as const,
    captionWidth: 138 as const,
    titlebarHeight: 34 as const,
    darkTheme: true,
    mounted: false,
  };
  const configure = jest.fn(() => receipt);
  const read = jest.fn(() => receipt);
  const api = createPublicApi(ui, {
    openLibrary: () => undefined,
    desktopFrame: { configureDesktopFrame: configure, readDesktopFrame: read },
  });
  const input = { ...receipt };
  expect(api.configureDesktopFrame?.(input)).toBe(receipt);
  expect(configure).toHaveBeenCalledWith(input);
  expect(api.readDesktopFrame?.()).toBe(receipt);
  expect(read).toHaveBeenCalledTimes(1);
  expect(configure).toHaveBeenCalledTimes(1);
  ui.dispose();
});
