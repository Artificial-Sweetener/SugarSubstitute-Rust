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
/** Read native selection coverage independently of mask loaders and other canvas consumers. */
import { imaging } from 'photoshop';
import type { PhotoshopDocument } from './PhotoshopCanvasIdentity.js';
import type { CanvasPixelRaster } from '../../../contracts/CanvasProtocol.generated.js';
import {
  encodeCanvasRaster,
  MAX_CANVAS_RASTER_BYTES,
} from '../../../shared/CanvasRasterEncoding.js';

/** Read a bounded full-document grayscale raster, including holes and feathered coverage. */
export async function readSelectionPixels(
  document: PhotoshopDocument,
  maximumSize: number,
): Promise<CanvasPixelRaster> {
  const scale = maximumSize
    ? Math.min(1, maximumSize / Math.max(document.width, document.height))
    : 1;
  const width = Math.max(1, Math.round(document.width * scale)),
    height = Math.max(1, Math.round(document.height * scale));
  if (width * height > MAX_CANVAS_RASTER_BYTES)
    throw new Error('The canvas selection exceeds the supported transfer size.');
  const pixels = new Uint8Array(width * height);
  const selection = document.selection.bounds;
  if (!selection) return encodeCanvasRaster(width, height, 'coverage8', pixels);
  const scaleX = width / document.width,
    scaleY = height / document.height;
  const left = Math.round(Math.max(0, selection.left) * scaleX),
    top = Math.round(Math.max(0, selection.top) * scaleY),
    right = Math.round(Math.min(document.width, selection.right) * scaleX),
    bottom = Math.round(Math.min(document.height, selection.bottom) * scaleY);
  if (right < left || bottom < top) return encodeCanvasRaster(width, height, 'coverage8', pixels);
  const image = await imaging.getSelection({
    documentID: document.id,
    sourceBounds: { left: 0, top: 0, right: document.width, bottom: document.height },
    // Photoshop crops to selection bounds before applying targetSize; scale that crop, not the frame.
    ...(maximumSize
      ? { targetSize: { width: Math.max(1, right - left), height: Math.max(1, bottom - top) } }
      : {}),
  });
  try {
    const data = await image.imageData.getData({ chunky: true, fullRange: true });
    const bounds = image.sourceBounds;
    const left = Math.round(bounds.left * scaleX),
      top = Math.round(bounds.top * scaleY);
    const sourceWidth = image.imageData.width,
      sourceHeight = image.imageData.height;
    const divisor =
      image.imageData.componentSize === 16
        ? 257
        : image.imageData.componentSize === 32
          ? 1 / 255
          : 1;
    for (let y = 0; y < sourceHeight; y++)
      for (let x = 0; x < sourceWidth; x++) {
        if (left + x < 0 || left + x >= width || top + y < 0 || top + y >= height) continue;
        pixels[(top + y) * width + left + x] = Math.max(
          0,
          Math.min(
            255,
            Math.round((data[(y * sourceWidth + x) * image.imageData.components] ?? 0) / divisor),
          ),
        );
      }
    return encodeCanvasRaster(width, height, 'coverage8', pixels);
  } finally {
    await image.imageData.dispose();
  }
}
