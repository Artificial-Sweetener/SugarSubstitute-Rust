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
/** Gate optional camera requests and retain only immutable native observations. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceViewportState,
} from '../contracts/CanvasSurface.js';

/** Require authoritative state for every optional camera command, including state reads. */
export function supportsCanvasSurfaceViewport(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  return (
    command.type !== 'surface.viewport.request' ||
    (capabilities.viewportState &&
      (command.command.type === 'read' ||
        capabilities.viewportCommands.includes(command.command.type)))
  );
}

/** Retain a native observation, including a shown comparison's B size, as one frozen value. */
export function snapshotCanvasSurfaceViewport(
  viewport: CanvasSurfaceViewportState,
): CanvasSurfaceViewportState {
  const { secondaryDimensions, ...state } = viewport;
  return Object.freeze({
    ...state,
    dimensions: Object.freeze({ ...viewport.dimensions }),
    ...(secondaryDimensions
      ? { secondaryDimensions: Object.freeze({ ...secondaryDimensions }) }
      : {}),
  });
}
