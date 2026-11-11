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
/** Project an observed processing-frame crop without resampling replacement pixels or coverage. */
import type { CanvasImageRectangle, CanvasImageRegion } from '../contracts/CanvasImageRegion.js';
import type { CanvasPixelRaster } from '../contracts/CanvasProtocol.generated.js';
import { decodeCanvasRaster, encodeCanvasRaster } from './CanvasRasterEncoding.js';

/** Use actual VAE dimensions rather than assuming a particular model's spatial compression. */
export function resolveCanvasImageRegion(
  image: CanvasPixelRaster,
  region: CanvasImageRegion,
): CanvasImageRegion {
  const frame = region.mapping.imageSize;
  if (
    region.outputFrame !== 'vae-center-crop' ||
    image.width > frame.width ||
    image.height > frame.height ||
    (image.width === frame.width && image.height === frame.height)
  )
    return region;
  return cropCanvasImageRegion(region, {
    left: Math.floor((frame.width - image.width) / 2),
    top: Math.floor((frame.height - image.height) / 2),
    width: image.width,
    height: image.height,
  });
}

/** Intersect image support and preserve its original document coordinate scale. */
export function cropCanvasImageRegion(
  region: CanvasImageRegion,
  crop: CanvasImageRectangle,
): CanvasImageRegion {
  const { imageSize, imageBounds: source, documentBounds: target } = region.mapping;
  if (
    region.mask.format !== 'coverage8' ||
    region.mask.width !== imageSize.width ||
    region.mask.height !== imageSize.height ||
    !Object.values(crop).every(Number.isSafeInteger) ||
    crop.left < 0 ||
    crop.top < 0 ||
    crop.width <= 0 ||
    crop.height <= 0 ||
    crop.left + crop.width > imageSize.width ||
    crop.top + crop.height > imageSize.height
  )
    throw new Error('Invalid processing frame crop.');
  const left = Math.max(source.left, crop.left),
    top = Math.max(source.top, crop.top);
  const right = Math.min(source.left + source.width, crop.left + crop.width);
  const bottom = Math.min(source.top + source.height, crop.top + crop.height);
  if (right <= left || bottom <= top)
    throw new Error('The processing crop removed its image support.');
  const original = decodeCanvasRaster(region.mask),
    mask = new Uint8Array(crop.width * crop.height);
  for (let y = 0; y < crop.height; y++) {
    const start = (crop.top + y) * imageSize.width + crop.left;
    mask.set(original.subarray(start, start + crop.width), y * crop.width);
  }
  const scaleX = target.width / source.width,
    scaleY = target.height / source.height;
  return Object.freeze({
    coverage: region.coverage,
    mask: Object.freeze(encodeCanvasRaster(crop.width, crop.height, 'coverage8', mask)),
    mapping: Object.freeze({
      imageSize: Object.freeze({ width: crop.width, height: crop.height }),
      imageBounds: Object.freeze({
        left: left - crop.left,
        top: top - crop.top,
        width: right - left,
        height: bottom - top,
      }),
      documentBounds: Object.freeze({
        left: target.left + (left - source.left) * scaleX,
        top: target.top + (top - source.top) * scaleY,
        width: (right - left) * scaleX,
        height: (bottom - top) * scaleY,
      }),
    }),
  });
}
