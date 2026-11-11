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
/** Expose document geometry independently of editor identity. */
import type {
  CanvasGeometryCommand,
  CanvasGeometryResult,
  CanvasDimensions,
  CanvasDimensionFieldValues,
  CanvasDimensionLimits,
  CanvasDocument,
  CanvasScaleOutput,
} from './CanvasProtocol.generated.js';
import type { CanvasCancellationSource } from './CanvasHost.js';

/** Read selection bounds and change bounds through one host-owned capability. */
export interface CanvasGeometryHost {
  readonly canvasId: string;
  execute(
    command: CanvasGeometryCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasGeometryResult>;
}

/** Describe a portable dimension calculation without transporting presentation to a native canvas. */
export interface CanvasDimensionProposal {
  readonly direction: 'from-target' | 'to-canvas';
  readonly reference: CanvasDimensions;
  readonly generation: CanvasDimensionFieldValues;
  readonly widthLimits: CanvasDimensionLimits;
  readonly heightLimits: CanvasDimensionLimits;
  readonly scale: string;
}
/** Capture a sizing interaction while the application owns stale-state validation and discovery. */
export interface CanvasDimensionConfiguration extends CanvasDimensionProposal {
  readonly document: CanvasDocument;
  readonly stage: 'factor' | 'confirm';
  readonly outputs: readonly CanvasScaleOutput[];
  readonly output: string | null;
}
/** Return an explicit menu decision without introducing UI commands into the canvas protocol. */
export interface CanvasDimensionChoice {
  readonly choice: 'apply' | 'discover' | 'cancel';
  readonly scale: string;
  readonly output: string | null;
}
