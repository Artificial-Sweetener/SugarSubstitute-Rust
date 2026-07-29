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
/** Define the renderer-neutral preview model consumed by a Cube surface. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';

export interface CubePreviewItem {
  key: string;
  url: string;
  label: string;
  sourceLocator?: string;
}

export interface CubeOutputPreview {
  id: string;
  label: string;
  items: CubePreviewItem[];
}

export interface CubePreviewSnapshot {
  outputs: CubeOutputPreview[];
  internalItems: CubePreviewItem[];
}

/** Provide media snapshots without coupling presentation to Comfy stores. */
export interface CubePreviewCatalog {
  snapshot(cube: CubeNode): CubePreviewSnapshot;
}

/** Keep preview sections aligned with the externally active output boundaries. */
export function filterCubePreviewOutputs(
  snapshot: CubePreviewSnapshot,
  outputSlots: readonly number[],
): CubePreviewSnapshot {
  return {
    outputs: outputSlots.flatMap((index) => {
      const output = snapshot.outputs[index];
      return output ? [output] : [];
    }),
    internalItems: snapshot.internalItems,
  };
}
