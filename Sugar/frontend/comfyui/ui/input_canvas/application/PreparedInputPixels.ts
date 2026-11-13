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
/** Retain processed input identity with its exact placement without teaching a canvas about Comfy nodes. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasImageMapping } from '../../../../../integrations/contracts/CanvasImageRegion.js';

/** Preserve the immutable asset that actual execution consumes and the frame its pixels represent. */
export interface PreparedInputPixels {
  readonly file: string;
  readonly classType: string;
  readonly field: string;
  readonly pixels: CanvasPixelRaster;
  readonly mapping: CanvasImageMapping;
  readonly sourceSize: { readonly width: number; readonly height: number };
}
