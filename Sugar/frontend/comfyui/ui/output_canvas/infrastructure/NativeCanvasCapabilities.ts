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
import {
  negotiatesCanvasSurfaceComparison,
  negotiatesCanvasSurfaceInspection,
  negotiatesCanvasSurfaceBackdrop,
} from '../../core/embedded/CanvasRuntime.js';
import type { CanvasCommand } from '../domain/CanvasViewPort.js';

const CAMERA_COMMANDS = ['fit', 'native', 'zoom', 'pan'] as const;
type CameraCommand = (typeof CAMERA_COMMANDS)[number];
const COMPARISON_COMMANDS = ['select-comparison', 'swap', 'wipe'] as const;

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

/** Admit native-frame material only through the negotiated geometry/frame acknowledgement contract. */
export function nativeCanvasBackdrop(host: CanvasSurfaceHost | null): boolean {
  return host !== null && negotiatesCanvasSurfaceBackdrop(host.capabilities);
}

/**
 * Advertise camera commands only with the viewport-state contract required by their shared
 * proxy, pair controls only with a negotiated pair-and-divider host, and the
 * overview grid only with a host that paints it. Linked inspection requires a host that
 * restores image-relative cameras. Comparison exit remains available during capability loss;
 * shared action admission separately checks whether comparison can be entered.
 */
export function nativeCanvasCommands(host: CanvasSurfaceHost | null): CanvasCommand['type'][] {
  const camera = host?.capabilities.viewportState
    ? host.capabilities.viewportCommands.filter((command): command is CameraCommand =>
        CAMERA_COMMANDS.some((name) => name === command),
      )
    : [];
  return [
    ...camera,
    'comparison',
    ...(nativeCanvasComparison(host) ? COMPARISON_COMMANDS : []),
    ...(nativeCanvasOverview(host) ? (['overview'] as const) : []),
    ...(nativeCanvasInspection(host) ? (['link'] as const) : []),
  ];
}

/** Report whether the host restores image-relative cameras and reports their centers. */
export function nativeCanvasInspection(host: CanvasSurfaceHost | null): boolean {
  return host !== null && negotiatesCanvasSurfaceInspection(host.capabilities);
}
