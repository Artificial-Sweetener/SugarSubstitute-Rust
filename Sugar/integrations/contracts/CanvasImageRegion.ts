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
/** Describe masked image placement in document pixels independently of any editor or execution engine. */
import type { CanvasPixelRaster } from './CanvasProtocol.generated.js';

/** Retain a rectangle's extent separately from the resolution used to sample it. */
export interface CanvasImageRectangle {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** Map the unpadded part of a processing raster onto its original document rectangle. */
export interface CanvasImageMapping {
  readonly imageSize: { readonly width: number; readonly height: number };
  readonly imageBounds: CanvasImageRectangle;
  readonly documentBounds: CanvasImageRectangle;
}

/** Freeze the exact return coverage in processing-image coordinates, including holes and soft edges. */
export interface CanvasImageRegion {
  /** Observe a verified VAE round trip's center crop from actual returned dimensions. */
  readonly outputFrame?: 'vae-center-crop';
  readonly mapping: CanvasImageMapping;
  readonly mask: CanvasPixelRaster;
  /** Finished composites already contain their feathering; raw proposals receive coverage once. */
  readonly coverage: 'mask' | 'support';
}

/** Supply a bounded transparent patch with an explicit document origin. */
export interface CanvasImagePatch {
  readonly pixels: CanvasPixelRaster;
  readonly position: { readonly left: number; readonly top: number };
}
