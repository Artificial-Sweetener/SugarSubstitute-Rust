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
/** Interpret reusable source pixels with the loader's actual channel convention. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import {
  usesNativeMaskChannels,
  nativeMaskOutputChannel,
  type InputLoader,
  type InputPreviewSettings,
} from './InputLoader.js';
import {
  decodeCanvasRaster,
  encodeCanvasRaster,
  canvasRasterByteLength,
} from '../../core/embedded/CanvasRuntime.js';

/** Give geometric selection coverage one fixed image representation independent of loader settings. */
export function maskSourceImage(source: CanvasPixelRaster): CanvasPixelRaster {
  if (source.format === 'rgba8') return source;
  const size = canvasRasterByteLength(source.width, source.height, 'rgba8');
  const values = decodeCanvasRaster(source),
    rgba = new Uint8Array(size);
  for (let index = 0; index < values.length; index++) {
    const value = values[index] ?? 0,
      offset = index * 4;
    rgba[offset] = rgba[offset + 1] = rgba[offset + 2] = value;
    rgba[offset + 3] = 255 - value;
  }
  return encodeCanvasRaster(source.width, source.height, 'rgba8', rgba);
}

/** Match Comfy's RGB extraction and inverse-alpha mask semantics without changing the input image. */
export function interpretMaskSource(source: CanvasPixelRaster, channel: string): CanvasPixelRaster {
  if (!['red', 'green', 'blue', 'alpha'].includes(channel))
    throw new Error('Unsupported mask channel.');
  const image = maskSourceImage(source),
    values = decodeCanvasRaster(image),
    coverage = new Uint8Array(image.width * image.height);
  const component = channel === 'alpha' ? 3 : channel === 'green' ? 1 : channel === 'blue' ? 2 : 0;
  for (let index = 0; index < coverage.length; index++) {
    const value = values[index * 4 + component] ?? 0;
    coverage[index] = channel === 'alpha' ? 255 - value : value;
  }
  return encodeCanvasRaster(image.width, image.height, 'coverage8', coverage);
}

/** Interpret known native coverage once and leave custom loader semantics to their native codec. */
export function nativeMaskSource(
  source: CanvasPixelRaster,
  loader: InputLoader,
  settings: InputPreviewSettings,
  imageMaskOutput: boolean,
): CanvasPixelRaster | null {
  const channel = imageMaskOutput ? nativeMaskOutputChannel(loader) : settings.inputs.channel;
  if (!imageMaskOutput && !usesNativeMaskChannels(loader)) return null;
  if (imageMaskOutput && channel === null) return null;
  if (typeof channel !== 'string') throw new Error('The mask channel must be an authored setting.');
  return interpretMaskSource(source, channel);
}
