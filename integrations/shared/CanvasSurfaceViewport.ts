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
  CanvasSurfaceCameraChange,
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceInspection,
  CanvasSurfaceViewportCommand,
  CanvasSurfaceViewportState,
} from '../contracts/CanvasSurface.js';

/**
 * Require authoritative state for every optional camera command, including state reads.
 *
 * A selection carrying an inspection restores a camera, so it requires the same declared
 * `inspect` command as an inspection applied to the shown image.
 */
export function supportsCanvasSurfaceViewport(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  if (command.type === 'surface.select')
    return command.inspection === undefined || negotiatesCanvasSurfaceInspection(capabilities);
  return (
    command.type !== 'surface.viewport.request' ||
    (capabilities.viewportState &&
      (command.command.type === 'read' ||
        capabilities.viewportCommands.includes(command.command.type)))
  );
}

/** Report whether the host restores image-relative inspections and reports viewport centers. */
export function negotiatesCanvasSurfaceInspection(
  capabilities: CanvasSurfaceCapabilities,
): boolean {
  return (
    (capabilities.schema === 2 || capabilities.schema === 3) &&
    capabilities.viewportState &&
    capabilities.viewportCommands.includes('inspect')
  );
}

/** Retain one image-relative inspection as a frozen value. */
export function snapshotCanvasSurfaceInspection(
  inspection: CanvasSurfaceInspection,
): CanvasSurfaceInspection {
  return Object.freeze({
    center: Object.freeze({ ...inspection.center }),
    span: Object.freeze({ ...inspection.span }),
  });
}

/** Retain one validated viewport command, including a nested inspection, as a frozen value. */
export function snapshotCanvasSurfaceViewportCommand(
  command: CanvasSurfaceViewportCommand,
): CanvasSurfaceViewportCommand {
  return command.type === 'inspect'
    ? Object.freeze({ ...command, inspection: snapshotCanvasSurfaceInspection(command.inspection) })
    : Object.freeze({ ...command });
}

/** Retain a native observation, including a shown comparison's B size, as one frozen value. */
export function snapshotCanvasSurfaceViewport(
  viewport: CanvasSurfaceViewportState,
): CanvasSurfaceViewportState {
  const { secondaryDimensions, center, ...state } = viewport;
  return Object.freeze({
    ...state,
    dimensions: Object.freeze({ ...viewport.dimensions }),
    ...(secondaryDimensions
      ? { secondaryDimensions: Object.freeze({ ...secondaryDimensions }) }
      : {}),
    ...(center ? { center: Object.freeze({ ...center }) } : {}),
  });
}

/** Retain one settled host-routed camera, including its nested viewport, as a frozen value. */
export function snapshotCanvasSurfaceCameraChange(
  change: CanvasSurfaceCameraChange,
): CanvasSurfaceCameraChange {
  return Object.freeze({
    ...change,
    address: Object.freeze({ ...change.address }),
    viewport: snapshotCanvasSurfaceViewport(change.viewport),
  });
}
