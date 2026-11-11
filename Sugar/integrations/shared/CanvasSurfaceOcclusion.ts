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
/** Gate page-published gesture occlusions on the host's native input routing declaration. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../contracts/CanvasSurface.js';

/** The schema's occlusion ceiling for one geometry update. */
export const CANVAS_SURFACE_OCCLUSIONS = 64;
/** The schema's coordinate and extent bound, in CSS pixels from the geometry origin. */
export const CANVAS_SURFACE_OCCLUSION_EXTENT = 1_000_000;

/**
 * Admit occlusions and the modal flag only for a host that routes native gestures itself.
 *
 * Hosts without the input declaration predate these optional geometry members, so a page
 * never sends them unknown fields.
 */
export function supportsCanvasSurfaceOcclusion(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  return (
    command.type !== 'surface.geometry' ||
    (command.geometry.occlusions === undefined && command.geometry.modal === undefined) ||
    capabilities.input
  );
}
