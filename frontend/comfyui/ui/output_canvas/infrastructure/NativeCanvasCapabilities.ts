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
/** Project negotiated native support into one immutable staged attachment identity. */
import type { CanvasSurfaceHost } from '../../../../../integrations/contracts/CanvasSurface.js';
import { negotiatesCanvasSurfaceComparison } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasCommand } from '../domain/CanvasViewPort.js';

const CAMERA_COMMANDS = ['fit', 'native', 'zoom', 'pan'] as const;
type CameraCommand = (typeof CAMERA_COMMANDS)[number];
const COMPARISON_COMMANDS = ['comparison', 'select-comparison', 'swap', 'wipe'] as const;

/** Rebuild a staged attachment when discovery changes its immutable command capabilities. */
export function nativeCanvasRendererKey(host: CanvasSurfaceHost | null): string {
  return `native:${JSON.stringify(host?.capabilities ?? null)}`;
}

/** Report whether the host can present an A/B pair and place its divider. */
export function nativeCanvasComparison(host: CanvasSurfaceHost | null): boolean {
  return host !== null && negotiatesCanvasSurfaceComparison(host.capabilities);
}

/** Report whether the host can paint the shared overview grid's cells. */
export function nativeCanvasOverview(host: CanvasSurfaceHost | null): boolean {
  return host?.capabilities.overview === true;
}

/** Report whether the host routes native canvas gestures against page-published occlusions. */
export function nativeCanvasInput(host: CanvasSurfaceHost | null): boolean {
  return host?.capabilities.input === true;
}

/**
 * Advertise camera commands only with the viewport-state contract required by their shared
 * proxy, the shared comparison commands only with a negotiated pair-and-divider host, and the
 * overview grid only with a host that paints it.
 */
export function nativeCanvasCommands(host: CanvasSurfaceHost | null): CanvasCommand['type'][] {
  const camera = host?.capabilities.viewportState
    ? host.capabilities.viewportCommands.filter((command): command is CameraCommand =>
        CAMERA_COMMANDS.some((name) => name === command),
      )
    : [];
  return [
    ...camera,
    ...(nativeCanvasComparison(host) ? COMPARISON_COMMANDS : []),
    ...(nativeCanvasOverview(host) ? (['overview'] as const) : []),
  ];
}
