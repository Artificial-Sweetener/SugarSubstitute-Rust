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
/** Prove prepared submission pixels survive unchanged captures without reapplying edits or cropping. */
import { jest } from '@jest/globals';
import { MaskInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/application/MaskInputPreparation.js';
import type { MaskEditingPort } from '../../../frontend/comfyui/ui/input_canvas/application/MaskEditingPort.js';
import { DEFAULT_MASK_EDITS } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskEditSettings.js';
import { editMaskInput } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';

test('reuses a frozen pair but recomputes for changed image content, mask coverage, settings or frame', async () => {
  const image = encodeCanvasRaster(
    4,
    2,
    'rgba8',
    Uint8Array.from({ length: 32 }, (_, i) => i),
  );
  const mask = encodeCanvasRaster(
    4,
    2,
    'coverage8',
    Uint8Array.from([0, 64, 255, 0, 0, 0, 128, 0]),
  );
  const prepare = jest.fn<MaskEditingPort['prepare']>(async (source, settings) =>
    editMaskInput(source, settings),
  );
  const owner = new MaskInputPreparation({ prepare }),
    settings = { ...DEFAULT_MASK_EDITS, crop_factor: 1 };
  const [first, concurrent] = await Promise.all([
    owner.prepare(image, mask, settings),
    owner.prepare({ ...image }, { ...mask }, { ...settings }),
  ]);
  expect(concurrent).toBe(first);
  expect(await owner.prepare({ ...image }, { ...mask }, settings)).toBe(first);
  expect(prepare).toHaveBeenCalledTimes(1);
  expect(first.mask.bounds).toEqual({ left: 1, top: 0, width: 2, height: 2 });
  expect([...decodeCanvasRaster(first.mask.mask)]).toEqual([64, 255, 0, 128]);
  expect([...decodeCanvasRaster(first.image).slice(0, 4)]).toEqual([4, 5, 6, 7]);
  expect(Object.isFrozen(first)).toBe(true);
  const newImage = encodeCanvasRaster(4, 2, 'rgba8', new Uint8Array(32).fill(99));
  expect((await owner.prepare(newImage, mask, settings)).image).not.toEqual(first.image);
  const newMask = encodeCanvasRaster(4, 2, 'coverage8', new Uint8Array(8).fill(255));
  expect((await owner.prepare(image, newMask, settings)).mask.bounds.width).toBe(4);
  expect(await owner.prepare(image, mask, { ...settings, feather: 1 })).not.toBe(first);
  await owner.prepare(
    { ...image, width: 2, height: 4 },
    { ...mask, width: 2, height: 4 },
    settings,
  );
  expect(prepare).toHaveBeenCalledTimes(5);
  await expect(owner.prepare(image, { ...mask, width: 2, height: 4 }, settings)).rejects.toThrow(
    'common input frame',
  );
});
