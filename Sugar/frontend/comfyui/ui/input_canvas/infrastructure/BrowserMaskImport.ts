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
/** Decode a chosen picture and resample it to a mask surface's size with the browser codec. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { grayscaleCoverage } from '../domain/MaskImport.js';

/** Produce coverage rasters at exactly the surface size. */
export class BrowserMaskImport {
  /** Draw on detached canvases of the page that owns the upload. */
  constructor(private readonly document: Document) {}

  /**
   * Stretch the picture to the surface, ignoring its aspect ratio, with smooth resampling, then
   * read its gray levels as coverage.
   */
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
      context.imageSmoothingEnabled = true;
      context.imageSmoothingQuality = 'high';
      context.drawImage(bitmap, 0, 0, width, height);
      const pixels = context.getImageData(0, 0, width, height).data;
      return encodeCanvasRaster(width, height, 'coverage8', grayscaleCoverage(pixels));
    } finally {
      canvas.width = canvas.height = 1;
      bitmap.close();
    }
  }
}
