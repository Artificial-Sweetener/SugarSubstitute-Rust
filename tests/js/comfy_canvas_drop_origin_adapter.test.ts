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
/** Verify Cube placement coordinates remain owned by one Comfy adapter. */

import { ComfyCanvasDropOriginAdapter } from '../../frontend/comfyui/ui/import/ComfyCanvasDropOriginAdapter.js';

test('prefers Comfy conversion of the last pointer position', () => {
  const adapter = new ComfyCanvasDropOriginAdapter({
    getCanvas: () => ({
      canvas: document.createElement('canvas'),
      last_mouse_position: [120, 240],
      convertCanvasToOffset: ([x, y]) => [x + 10, y + 20],
    }),
    logger: console,
  });

  expect(adapter.compute()).toEqual([130, 260]);
});

test('falls back through viewport center and transform offset', () => {
  const canvas = document.createElement('canvas');
  Object.defineProperty(canvas, 'getBoundingClientRect', {
    value: () => ({ width: 800, height: 600 }),
  });
  const viewportAdapter = new ComfyCanvasDropOriginAdapter({
    getCanvas: () => ({
      canvas,
      convertCanvasToOffset: ([x, y]) => [x / 2, y / 2],
    }),
    logger: console,
  });
  expect(viewportAdapter.compute()).toEqual([200, 150]);

  const offsetAdapter = new ComfyCanvasDropOriginAdapter({
    getCanvas: () => ({
      ds: { offset: [-30, -40] },
    }),
    logger: console,
  });
  expect(offsetAdapter.compute()).toEqual([30, 40]);
});
