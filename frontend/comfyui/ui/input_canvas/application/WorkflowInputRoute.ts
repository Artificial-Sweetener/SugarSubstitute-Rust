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
/** Own the Input/Output route and the active input surface and mask of each workflow. */
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import { workflowMaskKey } from '../domain/WorkflowSurfacePlan.js';
import type { WorkflowSurfaceState } from './WorkflowSurfaceProvisioning.js';

export type InputCanvasRoute = 'input' | 'output';
/** What a workflow remembers about its input canvas. */
export interface WorkflowInputSelection {
  readonly route: InputCanvasRoute;
  readonly surface: string | null;
  readonly mask: string | null;
}
/** Persist each workflow's selection where the workflow's own UI state lives. */
export interface WorkflowInputSelectionStore {
  read(workflow: string): WorkflowInputSelection | null;
  write(workflow: string, selection: WorkflowInputSelection): void;
}
/** The route control's view of the active workflow. */
export interface WorkflowInputRouteView extends WorkflowInputSelection {
  readonly available: boolean;
}
/** Synced surfaces and their change notifications. */
export interface WorkflowSurfaceSource {
  readonly state: WorkflowSurfaceState | null;
  subscribe(listener: () => void): () => void;
}

/** A workflow that remembers no selection opens on its input images, as it did in Substitute. */
const STARTING: WorkflowInputSelection = { route: 'input', surface: null, mask: null };

/**
 * Keep the canvas's visible route and input document in step with the remembered selection.
 *
 * Input is available only while the workflow has surfaces; otherwise the view stays on Output.
 * A workflow without a remembered selection starts on Input when it has surfaces.
 * A remembered surface or mask that disappears is cleared rather than reassigned. Showing Input
 * with no active surface activates the first surface and its first mask.
 */
export class WorkflowInputRoute {
  readonly #listeners = new Set<() => void>();
  readonly #stop: () => void;
  #workflow: string | null = null;
  #selection: WorkflowInputSelection = STARTING;
  #view: WorkflowInputRouteView | null = null;
  #activated: string | null = null;

  /** Follow synced surfaces and send activation through the canvas that holds them. */
  constructor(
    private readonly surfaces: WorkflowSurfaceSource,
    private readonly host: () => CanvasWorkflowHost | null,
    private readonly store: WorkflowInputSelectionStore,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = surfaces.subscribe(() => this.#reconcile());
    this.#reconcile();
  }

  /** The route view, or null when no workflow-surface canvas holds this workflow. */
  read(): WorkflowInputRouteView | null {
    return this.#view;
  }

  /** Observe route view changes. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Show Input or Output; Input is refused while the workflow has no surfaces. */
  chooseRoute(route: InputCanvasRoute): void {
    const state = this.surfaces.state;
    if (!state || (route === 'input' && !state.surfaces.length)) return;
    this.#choose({ ...this.#selection, route });
  }

  /** Show an image endpoint's surface with its first mask on the Input route. */
  activateImage(endpoint: string): boolean {
    const surface = this.surfaces.state?.surfaces.find((item) => item.image === endpoint);
    if (!surface) return false;
    this.#choose({ route: 'input', surface: surface.key, mask: surface.masks[0]?.key ?? null });
    return true;
  }

  /** Show a mask's surface, then make that mask active, on the Input route. */
  activateMask(endpoint: string, entry: string | null): boolean {
    const key = workflowMaskKey(endpoint, entry);
    const surface = this.surfaces.state?.surfaces.find((item) =>
      item.masks.some((mask) => mask.key === key || (entry === null && mask.endpoint === endpoint)),
    );
    const mask = surface?.masks.find(
      (item) => item.key === key || (entry === null && item.endpoint === endpoint),
    );
    if (!surface || !mask) return false;
    this.#choose({ route: 'input', surface: surface.key, mask: mask.key });
    return true;
  }

  /** Stop following surfaces. */
  dispose(): void {
    this.#stop();
    this.#listeners.clear();
  }

  /** Remember a user's choice for this workflow and apply it. */
  #choose(selection: WorkflowInputSelection): void {
    const workflow = this.#workflow;
    if (!workflow) return;
    this.#selection = selection;
    try {
      this.store.write(workflow, selection);
    } catch (error: unknown) {
      this.report(error);
    }
    this.#reconcile();
  }

  /** Validate the selection against synced surfaces, publish the view and activate on change. */
  #reconcile(): void {
    const state = this.surfaces.state;
    if (!state) {
      this.#workflow = null;
      this.#activated = null;
      this.#publish(null);
      return;
    }
    if (state.workflow !== this.#workflow) {
      this.#workflow = state.workflow;
      this.#activated = null;
      try {
        this.#selection = this.store.read(state.workflow) ?? STARTING;
      } catch (error: unknown) {
        this.report(error);
        this.#selection = STARTING;
      }
    }
    const available = state.surfaces.length > 0;
    let surface = state.surfaces.find((item) => item.key === this.#selection.surface) ?? null;
    let mask = surface?.masks.find((item) => item.key === this.#selection.mask)?.key ?? null;
    const route: InputCanvasRoute = available ? this.#selection.route : 'output';
    if (route === 'input' && !surface) {
      surface = state.surfaces[0] ?? null;
      mask = surface?.masks[0]?.key ?? null;
    }
    this.#selection = { route: this.#selection.route, surface: surface?.key ?? null, mask };
    this.#publish({
      route,
      surface: surface?.key ?? null,
      mask,
      available,
    });
    const command = {
      type: 'activate',
      workflow: state.workflow,
      route,
      surface: surface?.key ?? null,
      mask,
    } as const;
    const signature = JSON.stringify([state.canvasId, command]);
    if (signature === this.#activated) return;
    const host = this.host();
    if (!host || host.canvasId !== state.canvasId) return;
    this.#activated = signature;
    host.execute(command).then(
      (result) => {
        if (result.kind === 'failure') {
          if (this.#activated === signature) this.#activated = null;
          if (result.status !== 'cancelled') this.report(new Error(result.message));
        }
      },
      (error: unknown) => {
        if (this.#activated === signature) this.#activated = null;
        this.report(error);
      },
    );
  }

  /** Publish a changed view. */
  #publish(view: WorkflowInputRouteView | null): void {
    if (JSON.stringify(view) === JSON.stringify(this.#view)) return;
    this.#view = view;
    for (const listener of this.#listeners)
      try {
        listener();
      } catch (error: unknown) {
        this.report(error);
      }
  }
}
