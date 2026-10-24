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
/** Read canvas selections, layers and compositions through reusable source capabilities. */
import type {
  CanvasSourceChange,
  CanvasSourceQuery,
  CanvasSourceResult,
} from './CanvasProtocol.generated.js';

/** Expose full-document source rasters independently of their consumers.
 *
 * Layer catalogs use top-to-bottom preorder. Captures retain the document origin and aspect ratio;
 * transparent or unselected canvas areas remain in the frame. Image targets return straight sRGB
 * RGBA8 pixels; selection targets return coverage8. A batch shares one document content revision.
 */
export interface CanvasSourceHost {
  readonly canvasId: string;
  /** Capture requested targets at one document revision in full-canvas coordinates. */
  execute(query: CanvasSourceQuery): Promise<CanvasSourceResult>;
  /** Publish committed document or active-layer changes without capturing pixels during drawing. */
  subscribe(changed: (event: CanvasSourceChange) => void): () => void;
}
