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
/** Pack Nodes 1.0 Cube header actions without reserving hidden slots. */

import type { CubeFaceTitlebarActionKey } from './CubeFaceChromeActions.js';

export interface CubeCanvasChromeRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CubeCanvasChromeLayoutOptions {
  showCardMenu: boolean;
  titlebarActionKeys: readonly CubeFaceTitlebarActionKey[];
}

export interface CubeCanvasChromeLayout {
  editAction: CubeCanvasChromeRect;
  cardMenuAction: CubeCanvasChromeRect;
  chromeActions: Readonly<Partial<Record<CubeFaceTitlebarActionKey, CubeCanvasChromeRect>>>;
}

const FRAME_PADDING = 12;
const ACTION_HEIGHT = 24;
const ACTION_WIDTH = 28;
const ACTION_GAP = 6;

/** Pack visible actions leftward from Comfy's native editor action. */
export function layoutCubeCanvasChrome(
  header: CubeCanvasChromeRect,
  options: CubeCanvasChromeLayoutOptions,
): CubeCanvasChromeLayout {
  const y = header.y + (header.height - ACTION_HEIGHT) / 2;
  let right = header.x + header.width - FRAME_PADDING;
  const takeSlot = (): CubeCanvasChromeRect => {
    right -= ACTION_WIDTH;
    const slot = { x: right, y, width: ACTION_WIDTH, height: ACTION_HEIGHT };
    right -= ACTION_GAP;
    return slot;
  };

  const editAction = takeSlot();
  const chromeActions: Partial<Record<CubeFaceTitlebarActionKey, CubeCanvasChromeRect>> = {};
  for (const key of [...options.titlebarActionKeys].reverse()) {
    chromeActions[key] = takeSlot();
  }
  const cardMenuAction = options.showCardMenu
    ? takeSlot()
    : { x: right - ACTION_WIDTH, y, width: ACTION_WIDTH, height: ACTION_HEIGHT };

  return { editAction, cardMenuAction, chromeActions };
}
