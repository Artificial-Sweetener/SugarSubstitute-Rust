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
/** Keep ordinary Cube frame resizing out of the masonry allocation. */

import { clampCubePreviewWidth, type CubePreviewWidthRange } from './CubePreviewResizeGeometry.js';

/** Describe one rendered side-preview allocation. */
export interface CubePreviewFrameAllocation {
  frameWidth: number;
  previewWidth: number;
  range: CubePreviewWidthRange;
  active: boolean;
}

/** Apply outer width deltas to preview width while preserving divider choices. */
export class CubePreviewFrameResizePolicy {
  #previousFrameWidth: number | null = null;

  /** Return the preview width for the current frame and remember its baseline. */
  resolve(allocation: CubePreviewFrameAllocation): number {
    const frameWidth = finiteWidth(allocation.frameWidth);
    if (!allocation.active) {
      this.#previousFrameWidth = null;
      return allocation.previewWidth;
    }
    const previewWidth = clampCubePreviewWidth(allocation.previewWidth, allocation.range);
    const previousFrameWidth = this.#previousFrameWidth;
    this.#previousFrameWidth = frameWidth;
    if (previousFrameWidth === null) return previewWidth;
    return clampCubePreviewWidth(previewWidth + frameWidth - previousFrameWidth, allocation.range);
  }
}

/** Normalize host geometry without manufacturing a resize delta. */
function finiteWidth(value: number): number {
  return Number.isFinite(value) ? Math.max(1, value) : 1;
}
