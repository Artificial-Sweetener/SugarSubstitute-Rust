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
/** Compute Nodes 1.0 geometry for the optional-card reveal menu. */

import type { CubeFaceCardMenuEntry } from './CubeFaceCardPolicy.js';
import type { CubeCanvasLayout, CubeCanvasRect } from './CubeCanvasLayout.js';

const MENU_WIDTH = 220;
const MENU_PADDING = 6;
const MENU_ROW_HEIGHT = 26;

export interface CubeCanvasCardMenuItem {
  entry: CubeFaceCardMenuEntry;
  rect: CubeCanvasRect;
}

export interface CubeCanvasCardMenuLayout {
  rect: CubeCanvasRect;
  items: CubeCanvasCardMenuItem[];
}

/** Place the menu beneath its action while keeping it inside the Cube frame. */
export function computeCubeCanvasCardMenuLayout(
  layout: CubeCanvasLayout,
): CubeCanvasCardMenuLayout {
  const width = Math.min(MENU_WIDTH, Math.max(1, layout.frame.width - MENU_PADDING * 2));
  const height = Math.max(
    MENU_PADDING * 2 + MENU_ROW_HEIGHT,
    MENU_PADDING * 2 + layout.cardMenuEntries.length * MENU_ROW_HEIGHT,
  );
  const x = Math.max(
    layout.frame.x + MENU_PADDING,
    layout.cardMenuAction.x + layout.cardMenuAction.width - width,
  );
  const y = layout.cardMenuAction.y + layout.cardMenuAction.height + 4;
  const rect = { x, y, width, height };
  return {
    rect,
    items: layout.cardMenuEntries.map((entry, index) => ({
      entry,
      rect: {
        x: x + MENU_PADDING,
        y: y + MENU_PADDING + index * MENU_ROW_HEIGHT,
        width: width - MENU_PADDING * 2,
        height: MENU_ROW_HEIGHT,
      },
    })),
  };
}
