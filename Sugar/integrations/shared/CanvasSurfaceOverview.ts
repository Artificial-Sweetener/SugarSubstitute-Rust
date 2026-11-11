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
/** Gate and bound native overview grids beyond what the closed wire schema can express. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../contracts/CanvasSurface.js';
import type {
  CanvasSurfaceAddress,
  CanvasSurfaceOverviewCell,
} from '../contracts/CanvasSurfaceProtocol.generated.js';
import { CANVAS_SURFACE_MESSAGE_BYTES, canvasMessageFits } from './CanvasMessageSize.js';

/** The schema's cell ceiling, matching the eight visible cells of the shared overview window. */
export const CANVAS_SURFACE_OVERVIEW_CELLS = 8;

/** Admit an overview grid only when the host declared overview support. */
export function supportsCanvasSurfaceOverview(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  return command.type !== 'surface.overview' || capabilities.overview;
}

/** Require each overview cell to name a distinct result; JSON Schema cannot express keyed uniqueness. */
export function validCanvasSurfaceOverview(command: CanvasSurfaceCommand): boolean {
  if (command.type !== 'surface.overview') return true;
  return new Set(command.cells.map((cell) => cell.itemId)).size === command.cells.length;
}

/**
 * Report whether an encoded overview message fits the native surface message ceiling.
 *
 * Cells reuse the selection file reference, whose per-field bounds alone admit messages far
 * larger than the 32 KiB native ceiling, so the encoded size is the authoritative admission.
 */
export function canvasSurfaceOverviewMessageFits(message: unknown): boolean {
  return canvasMessageFits(message, CANVAS_SURFACE_MESSAGE_BYTES);
}

/**
 * Keep the leading cells whose request fits the native ceiling under any session envelope.
 *
 * The envelope is measured at its maximum encoded width for this address, so a trimmed list is
 * always admitted by the transport regardless of the request identity later assigned to it.
 */
export function fittingCanvasSurfaceOverviewCells(
  address: CanvasSurfaceAddress,
  cells: readonly CanvasSurfaceOverviewCell[],
): readonly CanvasSurfaceOverviewCell[] {
  const envelope = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session: 'x'.repeat(80),
    requestId: 2147483647,
    type: 'surface.overview',
    address,
    revision: Number.MAX_SAFE_INTEGER,
  };
  let count = Math.min(cells.length, CANVAS_SURFACE_OVERVIEW_CELLS);
  while (
    count > 0 &&
    !canvasSurfaceOverviewMessageFits({ ...envelope, cells: cells.slice(0, count) })
  )
    count--;
  return cells.slice(0, count);
}
