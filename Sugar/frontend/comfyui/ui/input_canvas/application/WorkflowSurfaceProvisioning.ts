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
/** Keep a workflow-surface canvas's input documents in step with the workflow graph. */
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasDocument,
  CanvasInputSource,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceFileReference } from '../../../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';
import {
  planWorkflowSurfaces,
  workflowSyncSurfaces,
  type PlannedSurface,
  type PlannedSurfaceMask,
  type RejectedMask,
  type WorkflowSurfaceFacts,
  type WorkflowSurfacePlan,
} from '../domain/WorkflowSurfacePlan.js';
import { WORKFLOW_SURFACE_COPY } from './WorkflowSurfaceCopy.js';
import { missingRegionMasks } from '../domain/RegionalPromptDemand.js';

/** Graph-derived facts and loader bindings of the active workflow. */
export interface WorkflowSurfaceGraph {
  /**
   * Stable workflow identity, or null when no workflow is loaded. `create` asks for a persisted
   * identity because the workflow has surfaces; otherwise the workflow is left unmodified.
   */
  workflow(create: boolean): string | null;
  facts(): WorkflowSurfaceFacts;
  /** The source binding owning a mask endpoint, used to bind its provisioned layers. */
  binding(endpoint: string): {
    provision(sources: ReadonlyMap<string | null, CanvasInputSource>): void;
  } | null;
  /** Observe topology, binding, file and sizing changes that can change the plan. */
  subscribe(changed: () => void): () => void;
}
/** Read a Comfy file's pixel size, or null when it cannot be read. */
export interface WorkflowFileDimensions {
  read(file: CanvasSurfaceFileReference): Promise<{ width: number; height: number } | null>;
}
/** Deliver non-blocking user feedback through the product's notification owner. */
export type WorkflowSurfaceNotice = (title: string, message: string) => void;

export interface ProvisionedMask extends PlannedSurfaceMask {
  readonly layerId: string;
}
export interface ProvisionedSurface extends Omit<PlannedSurface, 'masks'> {
  readonly document: CanvasDocument;
  readonly masks: readonly ProvisionedMask[];
}
/** The documents and layers the canvas holds for the active workflow. */
export interface WorkflowSurfaceState {
  readonly canvasId: string;
  readonly workflow: string;
  readonly surfaces: readonly ProvisionedSurface[];
  readonly rejected: readonly RejectedMask[];
}

/**
 * Plan surfaces from graph facts and replace the canvas's surface set for the active workflow.
 *
 * Change notifications coalesce into at most one sync in flight; a change during a sync schedules
 * exactly one follow-up. An unchanged plan is not resent. After a successful sync every provisioned
 * mask loader is bound to its mask layer, except loaders whose user chose another source.
 */
export class WorkflowSurfaceProvisioning {
  readonly #listeners = new Set<() => void>();
  readonly #stop: () => void;
  readonly #sizes = new Map<string, Promise<{ width: number; height: number } | null>>();
  readonly #rejectedInitials = new Set<string>();
  #state: WorkflowSurfaceState | null = null;
  #signature: string | null = null;
  #running: Promise<void> | null = null;
  #dirty = false;
  #disposed = false;

  /** Observe the graph and resolve the canvas lazily so a reconnected canvas is picked up. */
  constructor(
    private readonly graph: WorkflowSurfaceGraph,
    private readonly host: () => CanvasWorkflowHost | null,
    private readonly accent: () => string,
    private readonly files: WorkflowFileDimensions,
    private readonly notice: WorkflowSurfaceNotice,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = graph.subscribe(() => this.request());
  }

  /** The last successfully synced surfaces, or null when no canvas holds them. */
  get state(): WorkflowSurfaceState | null {
    return this.#state;
  }

  /** Observe synced-state changes. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Name a provisioned document so sources bound to it can be captured while it is inactive. */
  document(canvasId: string, documentId: string): CanvasDocument | null {
    const state = this.#state;
    if (state?.canvasId !== canvasId) return null;
    return state.surfaces.find((surface) => surface.document.id === documentId)?.document ?? null;
  }

  /** Schedule a coalesced sync after the current task's graph changes settle. */
  request(): Promise<void> {
    if (this.#disposed) return Promise.resolve();
    this.#dirty = true;
    this.#running ??= Promise.resolve()
      .then(() => this.#drain())
      .finally(() => {
        this.#running = null;
      });
    return this.#running;
  }

  /** Resend the current plan even if unchanged, after the canvas reconnects or the workflow loads. */
  refresh(): Promise<void> {
    this.#signature = null;
    return this.request();
  }

  /**
   * Refuse generation while a regional prompt declares more regions than its mask loader holds.
   *
   * Applies only while a workflow-surface canvas holds the workflow; pending syncs settle first so
   * the check reads the same graph the canvas holds.
   */
  async assertGenerationReady(): Promise<void> {
    await this.#running;
    if (!this.#state) return;
    for (const mask of this.graph.facts().masks) {
      const missing = missingRegionMasks(mask.regions, mask.entries.length);
      if (missing > 0)
        throw new Error(
          WORKFLOW_SURFACE_COPY.regionalMasksMissing(
            mask.label,
            mask.regions?.required ?? 0,
            mask.entries.length,
          ),
        );
    }
  }

  /** Replace a synced surface after the canvas resized it, keeping every other surface. */
  replaceSurface(key: string, document: CanvasDocument): void {
    const state = this.#state;
    if (!state) return;
    this.#publish({
      ...state,
      surfaces: state.surfaces.map((surface) =>
        surface.key === key ? { ...surface, document } : surface,
      ),
    });
  }

  /** Stop observing and drop state; a sync in flight finishes without publishing. */
  dispose(): void {
    this.#disposed = true;
    this.#stop();
    this.#listeners.clear();
    this.#state = null;
  }

  /** Sync until no change arrived during the previous sync. */
  async #drain(): Promise<void> {
    while (this.#dirty && !this.#disposed) {
      this.#dirty = false;
      try {
        await this.#sync();
      } catch (error: unknown) {
        this.report(error);
      }
    }
  }

  /** Plan, verify initial mask files, sync when changed, then bind provisioned loaders. */
  async #sync(): Promise<void> {
    const host = this.host();
    const planned = host
      ? planWorkflowSurfaces(this.graph.facts(), this.accent(), {
          synthetic: WORKFLOW_SURFACE_COPY.maskOnlySurface,
        })
      : null;
    const workflow = planned ? this.graph.workflow(planned.surfaces.length > 0) : null;
    if (!host || !planned || !workflow) {
      this.#signature = null;
      if (this.#state) this.#publish(null);
      return;
    }
    const plan = await this.#verified(planned);
    if (this.#disposed) return;
    const surfaces = workflowSyncSurfaces(plan);
    const signature = JSON.stringify([host.canvasId, workflow, surfaces]);
    if (signature === this.#signature) {
      // A dropped initial file leaves the plan unchanged; its loader still returns to its layer.
      // Rebinding drops the file from the facts, so the next plan drops nothing and stops here.
      const dropped = planned.surfaces.some((surface) =>
        surface.masks.some((mask) => mask.initial !== null),
      );
      if (dropped && this.#state) this.#bind(this.#state.canvasId, this.#state.surfaces);
      return;
    }
    await this.#followAuthoritySizes(host, workflow, plan);
    if (this.#disposed) return;
    const result = await host.execute({ type: 'sync', workflow, surfaces });
    if (this.#disposed) return;
    if (result.kind === 'failure') {
      this.#signature = null;
      if (result.status !== 'cancelled' && result.status !== 'unavailable')
        throw new Error(result.message || WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
      return;
    }
    if (result.kind !== 'synced') throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
    const synced = new Map(result.surfaces.map((surface) => [surface.key, surface]));
    const provisioned = plan.surfaces.map((surface): ProvisionedSurface => {
      const reply = synced.get(surface.key);
      const layers = new Map(reply?.masks.map((mask) => [mask.key, mask.layerId]));
      const masks = surface.masks.map((mask) => {
        const layerId = layers.get(mask.key);
        if (!layerId) throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
        return { ...mask, layerId };
      });
      if (!reply) throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
      return { ...surface, document: reply.document, masks };
    });
    this.#signature = signature;
    this.#publish({
      canvasId: host.canvasId,
      workflow,
      surfaces: provisioned,
      rejected: plan.rejected,
    });
    this.#bind(host.canvasId, provisioned);
  }

  /**
   * Resize existing mask-only surfaces whose size fields were edited directly, keeping their
   * pixels centred, so a sync never has to change the size of a surface the canvas holds.
   */
  async #followAuthoritySizes(
    host: CanvasWorkflowHost,
    workflow: string,
    plan: WorkflowSurfacePlan,
  ): Promise<void> {
    const state = this.#state;
    if (state?.workflow !== workflow || state.canvasId !== host.canvasId) return;
    for (const surface of plan.surfaces) {
      const current = state.surfaces.find((item) => item.key === surface.key)?.document;
      if (
        surface.base.kind !== 'blank' ||
        !current ||
        (current.width === surface.base.width && current.height === surface.base.height)
      )
        continue;
      const result = await host.execute({
        type: 'resize',
        workflow,
        surface: surface.key,
        width: surface.base.width,
        height: surface.base.height,
        mode: { kind: 'canvas', anchor: 'center' },
      });
      if (result.kind === 'failure') {
        if (result.status !== 'cancelled') throw new Error(result.message);
        continue;
      }
      const document =
        result.kind === 'synced'
          ? result.surfaces.find((item) => item.key === surface.key)?.document
          : undefined;
      if (document) this.replaceSurface(surface.key, document);
    }
  }

  /** Bind each mask loader's entries to their layers through the loader's own binding owner. */
  #bind(canvasId: string, surfaces: readonly ProvisionedSurface[]): void {
    const endpoints = new Map<string, Map<string | null, CanvasInputSource>>();
    for (const surface of surfaces)
      for (const mask of surface.masks) {
        const sources = endpoints.get(mask.endpoint) ?? new Map<string | null, CanvasInputSource>();
        sources.set(mask.entry, {
          kind: 'canvas',
          canvasId,
          documentId: surface.document.id,
          target: { kind: 'layer', layerId: mask.layerId },
        });
        endpoints.set(mask.endpoint, sources);
      }
    for (const [endpoint, sources] of endpoints)
      try {
        this.graph.binding(endpoint)?.provision(sources);
      } catch (error: unknown) {
        this.report(error);
      }
  }

  /**
   * Drop initial mask files whose size differs from their surface.
   *
   * A file whose size cannot be read is also dropped: a mask is only ever seeded from a file
   * that is known to match its image. The user is told once when they pick such a file for a
   * mask that already exists; a loader's preexisting file is skipped silently when its mask is
   * first created, so the mask simply starts blank.
   */
  async #verified(plan: WorkflowSurfacePlan): Promise<WorkflowSurfacePlan> {
    const existing = new Set(
      this.#state?.surfaces.flatMap((surface) => surface.masks.map((mask) => mask.key)),
    );
    const surfaces = await Promise.all(
      plan.surfaces.map(async (surface) => {
        if (!surface.masks.some((mask) => mask.initial)) return surface;
        const size =
          surface.base.kind === 'blank'
            ? { width: surface.base.width, height: surface.base.height }
            : await this.#size(surface.base.file);
        const masks = await Promise.all(
          surface.masks.map(async (mask) => {
            if (!mask.initial) return mask;
            const initial = await this.#size(mask.initial);
            if (initial && size && initial.width === size.width && initial.height === size.height)
              return mask;
            const key = JSON.stringify([mask.key, mask.initial, size]);
            if (existing.has(mask.key) && !this.#rejectedInitials.has(key)) {
              this.#rejectedInitials.add(key);
              if (initial && size)
                this.notice(
                  WORKFLOW_SURFACE_COPY.maskDimensionsMismatchTitle,
                  WORKFLOW_SURFACE_COPY.maskDimensionsMismatch(
                    `${String(initial.width)}x${String(initial.height)}`,
                    `${String(size.width)}x${String(size.height)}`,
                  ),
                );
              else
                this.notice(
                  WORKFLOW_SURFACE_COPY.maskDimensionsUnverifiedTitle,
                  WORKFLOW_SURFACE_COPY.maskDimensionsUnverified,
                );
            }
            return { ...mask, initial: null };
          }),
        );
        return { ...surface, masks };
      }),
    );
    return { ...plan, surfaces };
  }

  /** Read each file's size once per page; a failed read is retried on the next sync. */
  #size(file: CanvasSurfaceFileReference): Promise<{ width: number; height: number } | null> {
    const key = JSON.stringify(file);
    let size = this.#sizes.get(key);
    if (!size) {
      size = this.files.read(file).catch((error: unknown) => {
        this.#sizes.delete(key);
        this.report(error);
        return null;
      });
      this.#sizes.set(key, size);
    }
    return size;
  }

  /** Publish a new state to observers. */
  #publish(state: WorkflowSurfaceState | null): void {
    this.#state = state;
    for (const listener of this.#listeners)
      try {
        listener();
      } catch (error: unknown) {
        this.report(error);
      }
  }
}
