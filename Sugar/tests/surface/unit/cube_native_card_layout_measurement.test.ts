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
/** Safeguard one read phase before Cube layout writes and deferred native-card measurement. */
import { expect, jest, test } from '@jest/globals';
import {
  layoutCubeSurfaceDom,
  type CubeSurfaceDomLayoutOptions,
} from '../../../frontend/comfyui/ui/surface/CubeSurfaceDomLayout.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

/** Supply browser geometry only at the native-card measurement boundary. */
function fixture(): CubeSurfaceDomLayoutOptions {
  return {
    width: 600,
    state: createDefaultCubeSurfaceState(),
    cards: [
      {
        id: 'first',
        node: { id: 'first', size: [240, 300] },
        label: 'Native card',
        visible: true,
        enabled: true,
        showActivationControl: false,
      },
    ],
    cells: [document.createElement('article')],
    content: document.createElement('section'),
    masonry: document.createElement('div'),
    previewDivider: document.createElement('button'),
    previewRail: document.createElement('aside'),
  };
}

test('samples each native height once before applying any layout styles', () => {
  const options = fixture();
  const read = jest.fn(() => {
    expect(options.masonry.style.width).toBe('');
    expect(options.cells[0]?.style.width).toBe('');
    return 240;
  });
  Object.defineProperty(options.cells[0], 'offsetHeight', { get: read });
  const result = layoutCubeSurfaceDom(options);
  expect(read).toHaveBeenCalledTimes(1);
  expect(result.minimumHeight).not.toBeNull();
  expect(options.masonry.style.width).not.toBe('');
});

test('keeps fallback height provisional until a later layout samples a real native height', () => {
  const options = fixture();
  let measured = 0;
  Object.defineProperty(options.cells[0], 'offsetHeight', { get: () => measured });
  expect(layoutCubeSurfaceDom(options).minimumHeight).toBeNull();
  measured = 240;
  expect(layoutCubeSurfaceDom(options).minimumHeight).not.toBeNull();
});
