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
/** Literal source-owner specimens supply explicit LTR caret/line operands; browser geometry is never inferred by production code. */
import type { ReorderLayout } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import type { ReorderMeasuredGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import {
  prepareReorderPlacementTopology,
  type ReorderPlacementTopology,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-topology.ts';
/** These controlled line boxes and edge anchors are independent external geometry inputs to the real prepared owner. */
export function literalPlacementTopology(
  layout: ReorderLayout,
  geometry: ReorderMeasuredGeometry,
): ReorderPlacementTopology | null {
  const lines = new Map<string, number>();
  return prepareReorderPlacementTopology(
    layout,
    geometry.rects.map((rect) => {
      const key = `${rect.top}:${rect.bottom}`;
      let identity = lines.get(key);
      if (identity === undefined) {
        identity = lines.size;
        lines.set(key, identity);
      }
      return {
        rect,
        line: { top: rect.top, bottom: rect.bottom, identity },
        leading: rect.left,
        trailing: rect.right,
      };
    }),
    geometry.viewport,
  );
}
