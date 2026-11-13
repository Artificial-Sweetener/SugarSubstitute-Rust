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
/** Change how a mask loader's overlays look on the canvas that holds them. */
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import { workflowMaskKey } from '../domain/WorkflowSurfacePlan.js';
import type { WorkflowSurfaceSource } from './WorkflowInputRoute.js';

/** Persist a mask loader's visual opacity with that loader. */
export interface MaskOpacityStore {
  write(endpoint: string, opacity: number): void;
}

/**
 * Apply visual opacity to every mask of one loader as one undoable canvas step, and highlight a
 * hovered region. The stored opacity also reaches later syncs, so new masks of the loader match.
 */
export class WorkflowMaskAppearance {
  #hovered: string | null = null;

  /** Address masks through the synced state of the canvas that holds them. */
  constructor(
    private readonly surfaces: WorkflowSurfaceSource,
    private readonly host: () => CanvasWorkflowHost | null,
    private readonly store: MaskOpacityStore,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Whether the loader currently has masks on a workflow-surface canvas. */
  provisioned(endpoint: string): boolean {
    return Boolean(
      this.surfaces.state?.surfaces.some((surface) =>
        surface.masks.some((mask) => mask.endpoint === endpoint),
      ),
    );
  }

  /** Read the loader's synced opacity in 0..1, or null when it has no masks. */
  opacity(endpoint: string): number | null {
    for (const surface of this.surfaces.state?.surfaces ?? [])
      for (const mask of surface.masks) if (mask.endpoint === endpoint) return mask.opacity;
    return null;
  }

  /** Store and apply one loader's visual opacity; values outside 0..1 are rejected. */
  changeOpacity(endpoint: string, opacity: number): void {
    if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1)
      throw new RangeError('Mask opacity must be between 0 and 1.');
    const state = this.surfaces.state,
      host = this.host();
    const masks =
      state?.surfaces.flatMap((surface) =>
        surface.masks
          .filter((mask) => mask.endpoint === endpoint)
          .map((mask) => ({ surface: surface.key, mask: mask.key, color: mask.color, opacity })),
      ) ?? [];
    this.store.write(endpoint, opacity);
    if (!state || !host || host.canvasId !== state.canvasId || !masks.length) return;
    this.#send(host, { type: 'appearance', workflow: state.workflow, masks });
  }

  /** Highlight one region of a loader, or clear the highlight with a null entry. */
  hover(endpoint: string, entry: string | null): void {
    const state = this.surfaces.state,
      host = this.host();
    if (!state || !host || host.canvasId !== state.canvasId) return;
    const key = entry === null ? null : workflowMaskKey(endpoint, entry);
    const surface =
      state.surfaces.find((item) => item.masks.some((mask) => mask.endpoint === endpoint)) ?? null;
    if (!surface || key === this.#hovered) return;
    this.#hovered = key;
    this.#send(host, { type: 'hover', workflow: state.workflow, surface: surface.key, mask: key });
  }

  /** Report canvas refusals except cancellation. */
  #send(host: CanvasWorkflowHost, command: Parameters<CanvasWorkflowHost['execute']>[0]): void {
    host.execute(command).then(
      (result) => {
        if (result.kind === 'failure' && result.status !== 'cancelled')
          this.report(new Error(result.message));
      },
      (error: unknown) => this.report(error),
    );
  }
}
