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
/** Validate worker values before they cross into mask preparation policy or presentation. */
import { isRecord } from '../../types/common.js';
import {
  isCanvasPixelRaster,
  decodeCanvasRaster,
  type CanvasPixelRaster,
} from '../../core/embedded/CanvasRuntime.js';
import { readMaskEditSettings, type MaskEditSettings } from '../domain/MaskEditSettings.js';
import { maskInputResult, type EditedMaskInput } from '../domain/MaskInputEditing.js';

/** Carry only immutable source coverage and preparation settings into the worker. */
export interface MaskEditingRequest {
  readonly id: number;
  readonly source: CanvasPixelRaster;
  readonly settings: MaskEditSettings;
}

/** Admit worker requests independently of the browser's untyped message event. */
export function readMaskEditingRequest(value: unknown): MaskEditingRequest {
  if (
    !isRecord(value) ||
    typeof value.id !== 'number' ||
    !Number.isSafeInteger(value.id) ||
    value.id < 1 ||
    !isCanvasPixelRaster(value.source) ||
    value.source.format !== 'coverage8'
  )
    throw new Error('Invalid mask preparation request.');
  decodeCanvasRaster(value.source);
  return { id: value.id, source: value.source, settings: readMaskEditSettings(value.settings) };
}

/** Check the returned raster and crop against the exact source that requested them. */
export function readMaskEditingResult(value: unknown, source: CanvasPixelRaster): EditedMaskInput {
  if (
    !isRecord(value) ||
    !isCanvasPixelRaster(value.mask) ||
    !isRecord(value.bounds) ||
    typeof value.cropped !== 'boolean'
  )
    throw new Error('Invalid mask preparation response.');
  const { left, top, width, height } = value.bounds;
  if (
    typeof left !== 'number' ||
    typeof top !== 'number' ||
    typeof width !== 'number' ||
    typeof height !== 'number' ||
    ![left, top, width, height].every(Number.isSafeInteger) ||
    left < 0 ||
    top < 0 ||
    width < 1 ||
    height < 1 ||
    left + width > source.width ||
    top + height > source.height
  )
    throw new Error('Invalid mask preparation placement.');
  decodeCanvasRaster(value.mask);
  if (
    !value.cropped &&
    (left !== 0 || top !== 0 || width !== source.width || height !== source.height)
  )
    throw new Error('An uncropped mask must retain its source frame.');
  return maskInputResult(value.mask, { left, top, width, height }, value.cropped);
}
