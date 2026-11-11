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
/** Prepare paired input geometry and effective mask coverage without native canvas mutation. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasImageMapping,
  CanvasImageRectangle,
} from '../../../../../integrations/contracts/CanvasImageRegion.js';
import { decodeCanvasRaster, encodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { growMaskCoverage, featherMaskCoverage } from './MaskCoverageFilters.js';
import { readMaskEditSettings, type MaskEditSettings } from './MaskEditSettings.js';

/** Retain exact input coverage and its original-frame placement for previews and queued results. */
export interface EditedMaskInput {
  readonly mask: CanvasPixelRaster;
  readonly bounds: CanvasImageRectangle;
  readonly mapping: CanvasImageMapping;
  readonly cropped: boolean;
}

/** Derive every edit from the unchanged interpreted mask, never from a previously edited preview. */
export function editMaskInput(source: CanvasPixelRaster, edits: MaskEditSettings): EditedMaskInput {
  const settings = readMaskEditSettings(edits);
  if (source.format !== 'coverage8') throw new Error('Mask editing requires interpreted coverage.');
  const { width, height } = source;
  const grown = growMaskCoverage(decodeCanvasRaster(source), width, height, settings.mask_dilation);
  const bounds = settings.crop_factor
    ? cropMaskBounds(grown, width, height, settings.crop_factor)
    : null;
  const region = bounds ?? { left: 0, top: 0, width, height };
  const selected = bounds ? cropRasterBytes(grown, width, region, 1) : grown;
  const post = growMaskCoverage(selected, region.width, region.height, settings.post_dilation);
  const coverage = featherMaskCoverage(post, region.width, region.height, settings.feather);
  return maskInputResult(
    encodeCanvasRaster(region.width, region.height, 'coverage8', coverage),
    region,
    bounds !== null,
  );
}

/** Derive placement from admitted crop geometry instead of trusting transport-supplied mappings. */
export function maskInputResult(
  mask: CanvasPixelRaster,
  region: CanvasImageRectangle,
  cropped: boolean,
): EditedMaskInput {
  if (mask.format !== 'coverage8' || mask.width !== region.width || mask.height !== region.height)
    throw new Error('The prepared mask does not match its input frame.');
  const inputBounds = { left: 0, top: 0, width: region.width, height: region.height };
  return Object.freeze({
    mask: Object.freeze({ ...mask }),
    bounds: Object.freeze(region),
    mapping: Object.freeze({
      imageSize: Object.freeze({ width: region.width, height: region.height }),
      imageBounds: Object.freeze(inputBounds),
      documentBounds: Object.freeze(region),
    }),
    cropped,
  });
}

/** Crop any validated source raster to the mask's paired input region without resampling its pixels. */
export function cropInputRaster(
  source: CanvasPixelRaster,
  bounds: CanvasImageRectangle,
): CanvasPixelRaster {
  const { left, top, width, height } = bounds;
  if (
    ![left, top, width, height].every(Number.isSafeInteger) ||
    left < 0 ||
    top < 0 ||
    width <= 0 ||
    height <= 0 ||
    left + width > source.width ||
    top + height > source.height
  )
    throw new Error('The paired input crop exceeds its source frame.');
  if (!left && !top && width === source.width && height === source.height) return source;
  const channels = source.format === 'rgba8' ? 4 : 1;
  const output = cropRasterBytes(decodeCanvasRaster(source), source.width, bounds, channels);
  return encodeCanvasRaster(width, height, source.format, output);
}

/** Copy the same input rectangle without changing image channels or soft mask coverage. */
function cropRasterBytes(
  bytes: Uint8Array,
  sourceWidth: number,
  bounds: CanvasImageRectangle,
  channels: number,
): Uint8Array {
  const { left, top, width, height } = bounds,
    output = new Uint8Array(width * height * channels);
  for (let y = 0; y < height; y++) {
    const start = ((top + y) * sourceWidth + left) * channels;
    output.set(bytes.subarray(start, start + width * channels), y * width * channels);
  }
  return output;
}

/** Find nonzero soft-mask support and add context using SimpleSyrup's bounded crop-factor convention. */
function cropMaskBounds(
  source: Uint8Array,
  width: number,
  height: number,
  factor: number,
): CanvasImageRectangle | null {
  let left = width,
    top = height,
    right = 0,
    bottom = 0;
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
      if (source[y * width + x]) {
        left = Math.min(left, x);
        top = Math.min(top, y);
        right = Math.max(right, x + 1);
        bottom = Math.max(bottom, y + 1);
      }
  if (right <= left || bottom <= top) return null;
  const cropWidth = Math.min(width, Math.ceil((right - left) * factor));
  const cropHeight = Math.min(height, Math.ceil((bottom - top) * factor));
  return {
    left: Math.max(0, Math.min(width - cropWidth, Math.floor((left + right - cropWidth) / 2))),
    top: Math.max(0, Math.min(height - cropHeight, Math.floor((top + bottom - cropHeight) / 2))),
    width: cropWidth,
    height: cropHeight,
  };
}
