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
/** Negotiate bounded presentation-only acrylic without granting native privileges. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../contracts/CanvasSurface.js';

/** Report whether geometry can describe native acrylic behind transparent web controls. */
export function negotiatesCanvasSurfaceBackdrop(capabilities: CanvasSurfaceCapabilities): boolean {
  return capabilities.schema === 3 && capabilities.backdrop?.schema === 1;
}

/** Avoid sending later optional geometry members to hosts that do not implement them. */
export function supportsCanvasSurfaceBackdrop(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  return (
    command.type !== 'surface.geometry' ||
    command.geometry.backdrops === undefined ||
    negotiatesCanvasSurfaceBackdrop(capabilities)
  );
}
