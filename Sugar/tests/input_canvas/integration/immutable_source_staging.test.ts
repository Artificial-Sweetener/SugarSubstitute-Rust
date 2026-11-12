/** @jest-environment node */
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
/** Prove unchanged generation inputs reuse their encoded immutable Comfy upload. */
import { jest } from '@jest/globals';
import { BrowserCanvasSourceAssets } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/BrowserCanvasSourceAssets.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

test('shares encoded uploads across equivalent captures and retries a failed transport without caching failure', async () => {
  const transport = jest.fn(async (_input: string, _init: RequestInit) =>
    Response.json({ name: 'prepared.png', subfolder: 'sugarcubes/sources', type: 'input' }),
  );
  const assets = new BrowserCanvasSourceAssets(transport),
    image = encodeCanvasRaster(2, 2, 'rgba8', new Uint8Array(16).fill(128));
  const first = assets.stage(image);
  expect(assets.stage({ ...image })).toBe(first);
  expect(await first).toBe('sugarcubes/sources/prepared.png');
  await assets.stage({ ...image });
  expect(transport).toHaveBeenCalledTimes(1);
  expect(transport.mock.calls[0]?.[0]).toBe('/upload/image');
  const changed = encodeCanvasRaster(2, 2, 'rgba8', new Uint8Array(16).fill(255));
  transport.mockRejectedValueOnce(new Error('upload failed'));
  await expect(assets.stage(changed)).rejects.toThrow('upload failed');
  await assets.stage(changed);
  expect(transport).toHaveBeenCalledTimes(3);
});
