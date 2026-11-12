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
/** Prepare only mask-defined image content at its captured document placement, without comparing images. */
import type { CanvasPixelRaster } from '../contracts/CanvasProtocol.generated.js';
import type {
  CanvasImageMapping,
  CanvasImagePatch,
  CanvasImageRectangle,
  CanvasImageRegion,
} from '../contracts/CanvasImageRegion.js';
import {
  canvasRasterByteLength,
  decodeCanvasRaster,
  encodeCanvasRaster,
} from './CanvasRasterEncoding.js';
import { sampleCanvasComponent } from './CanvasImageSampling.js';
import { resolveCanvasImageRegion } from './CanvasImageRegionCrop.js';

/** Require coherent captured coverage while admitting outputs sampled at a different resolution. */
function validateMapping(image: CanvasPixelRaster, region: CanvasImageRegion): void {
  const { mapping, mask } = region;
  const { imageSize, imageBounds, documentBounds } = mapping;
  if (
    image.format !== 'rgba8' ||
    mask.format !== 'coverage8' ||
    mask.width !== imageSize.width ||
    mask.height !== imageSize.height
  )
    throw new Error(
      'The returned image or mask geometry does not match its captured processing frame.',
    );
  for (const rectangle of [imageBounds, documentBounds]) {
    if (
      !Object.values(rectangle).every(Number.isFinite) ||
      rectangle.width <= 0 ||
      rectangle.height <= 0 ||
      rectangle.width > 300000 ||
      rectangle.height > 300000 ||
      Math.abs(rectangle.left) > 300000 ||
      Math.abs(rectangle.top) > 300000
    )
      throw new Error('Invalid canvas return mapping.');
  }
  if (
    !Object.values(imageBounds).every(Number.isSafeInteger) ||
    imageBounds.left < 0 ||
    imageBounds.top < 0 ||
    imageBounds.left + imageBounds.width > imageSize.width ||
    imageBounds.top + imageBounds.height > imageSize.height
  )
    throw new Error('Invalid canvas return image mapping.');
}

/** Bound work to the actual coverage support, retaining holes instead of replacing the mask with a rectangle. */
function coverageBounds(
  mask: Uint8Array,
  width: number,
  bounds: CanvasImageRectangle,
): CanvasImageRectangle | null {
  let left = Infinity,
    top = Infinity,
    right = -Infinity,
    bottom = -Infinity;
  for (let y = bounds.top; y < bounds.top + bounds.height; y++)
    for (let x = bounds.left; x < bounds.left + bounds.width; x++)
      if ((mask[y * width + x] ?? 0) > 0) {
        left = Math.min(left, x);
        top = Math.min(top, y);
        right = Math.max(right, x + 1);
        bottom = Math.max(bottom, y + 1);
      }
  return Number.isFinite(left) ? { left, top, width: right - left, height: bottom - top } : null;
}

/** Project processing pixel edges into integer document coverage without a half-pixel placement drift. */
function documentRectangle(
  bounds: CanvasImageRectangle,
  mapping: CanvasImageMapping,
): CanvasImageRectangle {
  const source = mapping.imageBounds,
    target = mapping.documentBounds;
  const scaleX = target.width / source.width,
    scaleY = target.height / source.height;
  const borderX = scaleX === 1 ? 0 : 1,
    borderY = scaleY === 1 ? 0 : 1;
  const x0 = Math.max(source.left, bounds.left - borderX),
    y0 = Math.max(source.top, bounds.top - borderY);
  const x1 = Math.min(source.left + source.width, bounds.left + bounds.width + borderX);
  const y1 = Math.min(source.top + source.height, bounds.top + bounds.height + borderY);
  const left = Math.floor(target.left + (x0 - source.left) * scaleX);
  const top = Math.floor(target.top + (y0 - source.top) * scaleY);
  return {
    left,
    top,
    width: Math.ceil(target.left + (x1 - source.left) * scaleX) - left,
    height: Math.ceil(target.top + (y1 - source.top) * scaleY) - top,
  };
}

/** Sample returned pixels into the frozen input footprint without changing its mask or destination. */
export function prepareCanvasImagePatch(
  image: CanvasPixelRaster,
  region: CanvasImageRegion,
): CanvasImagePatch | null {
  region = resolveCanvasImageRegion(image, region);
  validateMapping(image, region);
  const pixels = decodeCanvasRaster(image),
    mask = decodeCanvasRaster(region.mask);
  const frame = region.mapping.imageSize;
  const support = coverageBounds(mask, frame.width, region.mapping.imageBounds);
  if (!support) return null;
  const rectangle = documentRectangle(support, region.mapping);
  const result = new Uint8Array(canvasRasterByteLength(rectangle.width, rectangle.height, 'rgba8'));
  const source = region.mapping.imageBounds,
    target = region.mapping.documentBounds;
  const scaleX = source.width / target.width,
    scaleY = source.height / target.height;
  const direct =
    scaleX === 1 && scaleY === 1 && Number.isInteger(target.left) && Number.isInteger(target.top);
  const imageScaleX = image.width / frame.width,
    imageScaleY = image.height / frame.height;
  const directImage = direct && imageScaleX === 1 && imageScaleY === 1;
  const imageBounds = {
    left: Math.floor(source.left * imageScaleX),
    top: Math.floor(source.top * imageScaleY),
    width:
      Math.ceil((source.left + source.width) * imageScaleX) - Math.floor(source.left * imageScaleX),
    height:
      Math.ceil((source.top + source.height) * imageScaleY) - Math.floor(source.top * imageScaleY),
  };
  let visible = false;
  for (let y = 0; y < rectangle.height; y++)
    for (let x = 0; x < rectangle.width; x++) {
      const sx = source.left + (rectangle.left + x + 0.5 - target.left) * scaleX - 0.5;
      const sy = source.top + (rectangle.top + y + 0.5 - target.top) * scaleY - 0.5;
      if (
        rectangle.left + x + 0.5 < target.left ||
        rectangle.top + y + 0.5 < target.top ||
        rectangle.left + x + 0.5 >= target.left + target.width ||
        rectangle.top + y + 0.5 >= target.top + target.height
      )
        continue;
      const ix = (sx + 0.5) * imageScaleX - 0.5,
        iy = (sy + 0.5) * imageScaleY - 0.5;
      const sourceOffset = (iy * image.width + ix) * 4;
      const coverage = direct
        ? (mask[sy * frame.width + sx] ?? 0)
        : sampleCanvasComponent(mask, frame.width, 1, 0, sx, sy, source);
      const alpha = directImage
        ? (pixels[sourceOffset + 3] ?? 0)
        : sampleCanvasComponent(pixels, image.width, 4, 3, ix, iy, imageBounds);
      const opacity = Math.round(
        alpha * (region.coverage === 'support' ? (coverage > 0 ? 1 : 0) : coverage / 255),
      );
      if (opacity === 0) continue;
      visible = true;
      const offset = (y * rectangle.width + x) * 4;
      for (let component = 0; component < 3; component++)
        result[offset + component] = directImage
          ? (pixels[sourceOffset + component] ?? 0)
          : Math.round(
              (sampleCanvasComponent(pixels, image.width, 4, component, ix, iy, imageBounds, true) *
                255) /
                alpha,
            );
      result[offset + 3] = opacity;
    }
  return visible
    ? {
        pixels: encodeCanvasRaster(rectangle.width, rectangle.height, 'rgba8', result),
        position: { left: rectangle.left, top: rectangle.top },
      }
    : null;
}
