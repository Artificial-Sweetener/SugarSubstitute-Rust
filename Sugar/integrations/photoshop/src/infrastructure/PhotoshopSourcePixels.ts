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
/** Read straight sRGB pixels from layers and compositions in the full Photoshop document frame. */
import { imaging } from 'photoshop';
import type { CanvasPixelRaster } from '../../../contracts/CanvasProtocol.generated.js';
import { placeCanvasRaster } from '../../../shared/CanvasRasterPlacement.js';
import { canvasRasterByteLength } from '../../../shared/CanvasRasterEncoding.js';
import type { PhotoshopDocument } from './PhotoshopCanvasIdentity.js';

/** Reconstruct native trimmed and pyramid-level bounds without scaling a crop to the whole canvas. */
export async function photoshopSourcePixels(
  document: PhotoshopDocument,
  layerId: string | null,
  maximumSize: number,
): Promise<CanvasPixelRaster> {
  const scale = maximumSize
    ? Math.min(1, maximumSize / Math.max(document.width, document.height))
    : 1;
  canvasRasterByteLength(
    Math.max(1, Math.round(document.width * scale)),
    Math.max(1, Math.round(document.height * scale)),
    'rgba8',
  );
  const image = await imaging.getPixels({
    documentID: document.id,
    ...(layerId === null ? {} : { layerID: Number(layerId) }),
    sourceBounds: { left: 0, top: 0, right: document.width, bottom: document.height },
    ...(maximumSize
      ? {
          targetSize: {
            width: Math.max(1, Math.round(document.width * scale)),
            height: Math.max(1, Math.round(document.height * scale)),
          },
        }
      : {}),
    colorSpace: 'RGB',
    colorProfile: 'sRGB IEC61966-2.1',
    componentSize: 8,
    applyAlpha: false,
  });
  // Empty native layers can omit raster data despite the SDK's nonnullable declaration.
  const nativeData: unknown = image.imageData;
  if (nativeData === null || nativeData === undefined)
    return placeCanvasRaster(
      document,
      {
        width: 0,
        height: 0,
        bounds: { left: 0, top: 0, right: 0, bottom: 0 },
        bytes: new Uint8Array(),
      },
      'rgba8',
      maximumSize,
    );
  const data = image.imageData;
  try {
    if (data.componentSize !== 8 || data.components < 1 || data.components > 4)
      throw new Error('Photoshop returned an unsupported raster packing.');
    const values = await data.getData({ chunky: true, fullRange: true });
    const bytes = new Uint8Array(canvasRasterByteLength(data.width, data.height, 'rgba8'));
    if (values.length !== data.width * data.height * data.components)
      throw new Error('Photoshop returned an incomplete raster.');
    for (let pixel = 0; pixel < data.width * data.height; pixel++) {
      const source = pixel * data.components,
        target = pixel * 4;
      bytes[target] = values[source] ?? 0;
      bytes[target + 1] = values[source + (data.components >= 3 ? 1 : 0)] ?? 0;
      bytes[target + 2] = values[source + (data.components >= 3 ? 2 : 0)] ?? 0;
      bytes[target + 3] =
        data.components === 4
          ? (values[source + 3] ?? 0)
          : data.components === 2
            ? (values[source + 1] ?? 0)
            : 255;
    }
    if (!Number.isSafeInteger(image.level) || image.level < 0 || image.level > 30)
      throw new Error('Photoshop returned an invalid raster pyramid level.');
    const factor = 2 ** image.level;
    const bounds = image.sourceBounds;
    return placeCanvasRaster(
      document,
      {
        width: data.width,
        height: data.height,
        bytes,
        bounds: {
          left: bounds.left * factor,
          top: bounds.top * factor,
          right: bounds.right * factor,
          bottom: bounds.bottom * factor,
        },
      },
      'rgba8',
      maximumSize,
    );
  } finally {
    await data.dispose();
  }
}
