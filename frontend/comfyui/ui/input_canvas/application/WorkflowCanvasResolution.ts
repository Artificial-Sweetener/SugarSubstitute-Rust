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
/** Change a mask-only surface's resolution and write it back into the nodes that size it. */
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasDocument,
  CanvasWorkflowResizeMode,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { DimensionAuthorityNode } from '../domain/CanvasDimensionAuthority.js';
import { WORKFLOW_SURFACE_COPY } from './WorkflowSurfaceCopy.js';
import type { ProvisionedSurface, WorkflowSurfaceState } from './WorkflowSurfaceProvisioning.js';

/** Inclusive pixel range offered for a mask-only canvas. */
export const CANVAS_RESOLUTION_LIMITS = Object.freeze({ minimum: 64, maximum: 32768, step: 8 });

/** Write one size into every authority field pair. */
export interface DimensionAuthorityWriter {
  write(nodes: readonly DimensionAuthorityNode[], width: number, height: number): void;
}
/** Synced surfaces whose documents a resize replaces. */
export interface ResizableSurfaces {
  readonly state: WorkflowSurfaceState | null;
  replaceSurface(key: string, document: CanvasDocument): void;
}

/**
 * Resize the canvas first and write the workflow only after the canvas accepted the new size,
 * so the workflow never names a size its canvas does not have.
 */
export class WorkflowCanvasResolution {
  /** Address surfaces through the canvas that holds them. */
  constructor(
    private readonly surfaces: ResizableSurfaces,
    private readonly host: () => CanvasWorkflowHost | null,
    private readonly writer: DimensionAuthorityWriter,
  ) {}

  /** Find the mask-only surface that a node's width or height field sizes. */
  surface(node: string, field: string): ProvisionedSurface | null {
    return (
      this.surfaces.state?.surfaces.find((surface) =>
        surface.authority?.some(
          (item) => item.node === node && (item.width === field || item.height === field),
        ),
      ) ?? null
    );
  }

  /** Resize one mask-only surface; failures leave the workflow unchanged and explain why. */
  async resize(
    key: string,
    width: number,
    height: number,
    mode: CanvasWorkflowResizeMode,
  ): Promise<void> {
    for (const value of [width, height])
      if (
        !Number.isSafeInteger(value) ||
        value < CANVAS_RESOLUTION_LIMITS.minimum ||
        value > CANVAS_RESOLUTION_LIMITS.maximum
      )
        throw new RangeError(WORKFLOW_SURFACE_COPY.resizeInvalid);
    const state = this.surfaces.state,
      host = this.host();
    const surface = state?.surfaces.find((item) => item.key === key);
    if (!state || !host || host.canvasId !== state.canvasId || !surface?.authority)
      throw new Error(WORKFLOW_SURFACE_COPY.resizeStale);
    if (surface.document.width === width && surface.document.height === height) return;
    const result = await host.execute({
      type: 'resize',
      workflow: state.workflow,
      surface: key,
      width,
      height,
      mode,
    });
    const synced =
      result.kind === 'synced' ? result.surfaces.find((item) => item.key === key) : undefined;
    if (!synced) throw new Error(WORKFLOW_SURFACE_COPY.resizeFailed);
    this.surfaces.replaceSurface(key, synced.document);
    try {
      this.writer.write(surface.authority, width, height);
    } catch (error: unknown) {
      throw new Error(WORKFLOW_SURFACE_COPY.resizeWriteFailed, { cause: error });
    }
  }
}
