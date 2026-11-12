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
/** Pack section runs into one continuous masonry coordinate system. */
import { computeCubeMasonry, } from '../CubeMasonryLayout.js';
import { cubeStackReservedWidth } from '../stack/CubeStackMetrics.js';
/** Align sections and navigation below the panel edge; native toolbars occupy separate flow. */
export const WORKFLOW_MASONRY_TOP_INSET = 12;
/** Minimum card column width of the native appearance, in CSS pixels. */
export const WORKFLOW_MASONRY_COLUMN_WIDTH = 400;
/** Gap between native-appearance card columns, in CSS pixels. */
export const WORKFLOW_MASONRY_GAP = 8;
/** Default padding on each side of the masonry content, in CSS pixels. */
export const WORKFLOW_MASONRY_INLINE_PADDING = 20;
/**
 * Size the editor beside the output canvas to the expanded cube stack plus two card columns.
 *
 * This is the editor width of every workflow whose divider the user has not moved.
 */
export const WORKFLOW_MASONRY_DEFAULT_EDITOR_WIDTH = cubeStackReservedWidth(false, false, false) +
    2 * WORKFLOW_MASONRY_INLINE_PADDING +
    2 * WORKFLOW_MASONRY_COLUMN_WIDTH +
    WORKFLOW_MASONRY_GAP;
/** Reset occupied column heights at each section while preserving shared column widths. */
export function computeWorkflowMasonry(sections, options) {
    const result = { height: 0, sections: [], placements: [] };
    for (const section of sections) {
        const grid = computeCubeMasonry(section.cards, options);
        const y = result.height;
        const headerHeight = options.gap === 0 ? 36 : 44;
        result.sections.push({ id: section.id, y, height: grid.height + headerHeight });
        result.placements.push(...grid.placements.map((card) => ({ ...card, y: y + headerHeight + card.y })));
        result.height += headerHeight + grid.height + (options.gap === 0 ? 0 : 24);
    }
    return result;
}
