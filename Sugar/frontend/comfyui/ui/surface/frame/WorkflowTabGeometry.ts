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
/** Resolve workflow-tab positions that only the tab strip asks about. */
import { slotOffsets } from '../../interaction/reorder/ReorderSlots.js';


/** Resolve a scroll position against actual tab extents, including unequal name-sized tabs. */
export function workflowTabAtOffset(widths: readonly number[], position: number): number {
  const offsets = slotOffsets(widths);
  const index = widths.findIndex((_width, index) => (offsets[index + 1] ?? 0) > position + 0.5);
  return index < 0 ? Math.max(0, widths.length - 1) : index;
}
