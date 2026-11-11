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
/** Gate optional A/B comparison operations on the negotiated surface declaration. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../contracts/CanvasSurface.js';

/**
 * Admit a comparison pair or wipe divider only when the host declared comparison support.
 *
 * Wipe also requires its viewport command declaration, which the viewport gate checks; a host
 * that declares comparison declares wipe as well, so either omission refuses the operation.
 */
export function supportsCanvasSurfaceComparison(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  const comparison =
    (command.type === 'surface.select' && command.comparison !== undefined) ||
    (command.type === 'surface.viewport.request' && command.command.type === 'wipe');
  return !comparison || capabilities.comparison;
}

/** Report whether pair selection and divider placement are both negotiated. */
export function negotiatesCanvasSurfaceComparison(
  capabilities: CanvasSurfaceCapabilities,
): boolean {
  return (
    capabilities.comparison &&
    capabilities.viewportState &&
    capabilities.viewportCommands.includes('wipe')
  );
}
