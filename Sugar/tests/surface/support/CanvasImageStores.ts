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
/** Supply isolated page-lifetime image stores to graph-bound Cube presentation proofs. */
import { CubeIconImageStore } from '../../../frontend/comfyui/ui/core/CubeIconImageStore.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { CubePreviewImageStore } from '../../../frontend/comfyui/ui/surface/CubeCanvasPreviewImageCache.js';
import type { CubeCanvasImageStores } from '../../../frontend/comfyui/ui/surface/CubeCanvasImageStores.js';

/** Create stores bound to a private allowance so no proof shares decoded state with another. */
export function testCanvasImages(memory = new MemoryBudgetRegistry()): CubeCanvasImageStores {
  return { icons: new CubeIconImageStore(memory), previews: new CubePreviewImageStore(memory) };
}
