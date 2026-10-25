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
/** Translate bounded browser image decoding into the shared editor-independent region preparation. */
import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { prepareCanvasImagePatch, encodeCanvasImage } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasImageRegion } from '../../../../../integrations/contracts/CanvasImageRegion.js';
import type { CanvasCancellationSource } from '../../../../../integrations/contracts/CanvasHost.js';
import { encodeSourcePng } from '../../core/embedded/CanvasPngEncoding.js';
import type { PreparedGenerationImage } from '../domain/CanvasGenerationImages.js';

/** Decode once, keep replacement pixels bounded, and release all native raster allocations. */
export async function browserCanvasImageRegion(
  blob: Blob,
  region: CanvasImageRegion,
  cancellation: CanvasCancellationSource,
): Promise<PreparedGenerationImage | null> {
  const bitmap = await createImageBitmap(blob);
  try {
    if (cancellation.cancelled()) return null;
    if (bitmap.width * bitmap.height * 4 > MAX_CANVAS_RASTER_BYTES)
      throw new Error('The generated image exceeds its decoded pixel budget.');
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    try {
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Generated image pixel decoding is unavailable.');
      context.drawImage(bitmap, 0, 0);
      const pixels = encodeCanvasRaster(
        bitmap.width,
        bitmap.height,
        'rgba8',
        new Uint8Array(context.getImageData(0, 0, bitmap.width, bitmap.height).data),
      );
      const patch = prepareCanvasImagePatch(pixels, region);
      if (!patch || cancellation.cancelled()) return null;
      const png = await encodeSourcePng(patch.pixels);
      if (cancellation.cancelled()) return null;
      return {
        image: encodeCanvasImage(new Uint8Array(await png.arrayBuffer())),
        position: patch.position,
      };
    } finally {
      canvas.width = canvas.height = 1;
    }
  } finally {
    bitmap.close();
  }
}
