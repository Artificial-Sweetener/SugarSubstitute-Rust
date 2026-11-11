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
/** Name the page-lifetime image stores that graph-bound Cube presentation borrows. */
import type { CubeIconImageStore } from '../core/CubeIconImageStore.js';
import type { CubePreviewImageStore } from './CubeCanvasPreviewImageCache.js';

/** Decoded images that survive workflow runtimes; hosts never own their lifetime. */
export interface CubeCanvasImageStores {
  readonly icons: CubeIconImageStore;
  readonly previews: CubePreviewImageStore;
}
