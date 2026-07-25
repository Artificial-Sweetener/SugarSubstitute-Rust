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
/** Verify typed capture and restoration at Comfy's canvas-view boundary. */

import { jest } from '@jest/globals';
import { ComfyCanvasViewStateAdapter } from '../../frontend/comfyui/ui/surface/ComfyCanvasViewStateAdapter.js';

test('captures and restores one finite Comfy canvas transform', () => {
  const canvas = { ds: { scale: 0.6, offset: [120, -45] } };
  const setDirtyCanvas = jest.fn();
  const adapter = new ComfyCanvasViewStateAdapter(canvas, setDirtyCanvas);
  const captured = adapter.capture();

  canvas.ds.scale = 1.4;
  canvas.ds.offset = [-300, 80];
  if (!captured) throw new Error('Expected a captured canvas view.');
  adapter.restore(captured);

  expect(canvas.ds).toEqual({ scale: 0.6, offset: [120, -45] });
  expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
});

test('rejects incomplete canvas transforms at the host boundary', () => {
  const adapter = new ComfyCanvasViewStateAdapter(
    { ds: { scale: Number.NaN, offset: [0] } },
    jest.fn(),
  );

  expect(adapter.capture()).toBeNull();
});

test('supports the typed-array offset exposed by LiteGraph', () => {
  const offset = new Float32Array([35, -70]);
  const canvas = { ds: { scale: 0.8, offset } };
  const adapter = new ComfyCanvasViewStateAdapter(canvas, jest.fn());
  const captured = adapter.capture();

  offset[0] = 400;
  offset[1] = 500;
  if (!captured) throw new Error('Expected a captured typed-array canvas view.');
  adapter.restore(captured);

  expect([...offset]).toEqual([35, -70]);
});
