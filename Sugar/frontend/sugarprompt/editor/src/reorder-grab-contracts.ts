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
/** The finite readonly grab result is shared by capture, materialization and feedback without a dependency on their implementations. */
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';

/** An untransferred external paint node retains its original viewport CSS-pixel footprint and press hotspot. */
export interface ReorderGrabPaint {
  readonly node: HTMLElement;
  readonly geometry: ReorderGrabGeometry;
}
