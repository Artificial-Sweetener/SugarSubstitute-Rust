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
/** Decode a chosen picture and draw it into a mask surface's size with the browser codec. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { aspectFit, grayscaleMask } from '../domain/MaskImport.js';

/** Produce opaque grayscale rasters at exactly the surface size. */
export class BrowserMaskImport {
  /** Draw on detached canvases of the page that owns the upload. */
  constructor(private readonly document: Document) {}

  /** Aspect-fit the picture onto black, then convert it to grayscale coverage. */
  async raster(file: Blob, width: number, height: number): Promise<CanvasPixelRaster> {
    if (width * height * 4 > MAX_CANVAS_RASTER_BYTES)
      throw new Error('The mask exceeds its decoded pixel budget.');
    const bitmap = await createImageBitmap(file);
    const canvas = this.document.createElement('canvas');
    try {
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Mask decoding is unavailable.');
      context.fillStyle = '#000';
      context.fillRect(0, 0, width, height);
      context.imageSmoothingQuality = 'high';
      const fit = aspectFit(bitmap.width, bitmap.height, width, height);
      context.drawImage(bitmap, fit.left, fit.top, fit.width, fit.height);
      const pixels = context.getImageData(0, 0, width, height).data;
      return encodeCanvasRaster(width, height, 'rgba8', grayscaleMask(pixels));
    } finally {
      canvas.width = canvas.height = 1;
      bitmap.close();
    }
  }
}
