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
import {
  computeCubeMasonry,
  type CubeMasonryCard,
  type CubeMasonryOptions,
  type CubeMasonryPlacement,
} from '../CubeMasonryLayout.js';

export interface WorkflowMasonryLayoutSection {
  id: string;
  cards: readonly CubeMasonryCard[];
}

export interface WorkflowMasonryLayoutResult {
  height: number;
  sections: { id: string; y: number; height: number }[];
  placements: CubeMasonryPlacement[];
}

/** Reset occupied column heights at each section while preserving shared column widths. */
export function computeWorkflowMasonry(
  sections: readonly WorkflowMasonryLayoutSection[],
  options: CubeMasonryOptions,
): WorkflowMasonryLayoutResult {
  const result: WorkflowMasonryLayoutResult = { height: 0, sections: [], placements: [] };
  for (const section of sections) {
    const grid = computeCubeMasonry(section.cards, options);
    const y = result.height;
    const headerHeight = 44;
    result.sections.push({ id: section.id, y, height: grid.height + headerHeight });
    result.placements.push(
      ...grid.placements.map((card) => ({ ...card, y: y + headerHeight + card.y })),
    );
    result.height += headerHeight + grid.height + 24;
  }
  return result;
}
