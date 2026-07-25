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
/** Define the stable application contract shared by Cube import collaborators. */

import type { Vec2 } from '../types/common.js';
import type { GraphId } from '../types/graph.js';

export interface ImportResult {
  success: boolean;
  summary: string;
  message: string;
  warnings: string[];
  missingTypes: string[];
  nodesAdded: number;
  markersAdded: number;
  connectionsMade: number;
  primaryNodeId: GraphId | null;
  bounds: { minX: number; minY: number; maxX: number; maxY: number } | null;
}

export interface ImportOptions {
  dropOrigin?: Vec2;
  focus?: boolean;
  setBusy?(busy: boolean): void;
  button?: { enabled: boolean; element: HTMLElement };
}
