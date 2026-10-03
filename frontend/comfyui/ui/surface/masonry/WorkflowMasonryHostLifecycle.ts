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
/** Adapt Comfy renderer, graph, command, and DOM lifecycles to workflow masonry. */
import type { ComfyApplication, ComfyGraph } from '../../types/graph.js';
import { isRecord } from '../../types/common.js';
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import {
  createComfyRendererModeChangeSource,
  resolveComfyRendererMode,
} from '../../core/ComfyRendererMode.js';
import { CubeSurfaceRendererLifecycle } from '../CubeSurfaceRendererLifecycle.js';
import {
  NativeSubgraphChangeObserver,
  type ObservableNativeSubgraph,
} from '../NativeSubgraphChangeObserver.js';
import { WorkflowMasonryController } from './WorkflowMasonryController.js';
import {
  projectWorkflowMasonry,
  type WorkflowMasonryConnection,
} from './WorkflowMasonryProjection.js';
import { ensureWorkflowMasonryStyles } from './WorkflowMasonryStyles.js';
import { WorkflowMasonryToolbarButton } from './WorkflowMasonryToolbarButton.js';

export interface WorkflowMasonryHostLifecycleOptions {
  app: ComfyApplication | null;
  document: Document;
  getCubes(): readonly CubeNode[];
  getCardState(): CubeFaceCardStateService | null;
  getConnections(graph: ComfyGraph): readonly WorkflowMasonryConnection[];
  alert(title: string, message: string): Promise<unknown>;
  reportError(error: unknown): void;
  logger: Pick<Console, 'warn'>;
}

/** Compose focused owners and keep host registration free of layout behavior. */
export class WorkflowMasonryHostLifecycle {
  readonly #options: WorkflowMasonryHostLifecycleOptions;
  readonly #controller: WorkflowMasonryController;
  readonly #renderers: CubeSurfaceRendererLifecycle;
  readonly #toolbar: WorkflowMasonryToolbarButton;
  readonly #graphs = new Map<ObservableNativeSubgraph, NativeSubgraphChangeObserver>();
  #unsubscribeRenderer: (() => void) | null = null;
  #unsubscribeCards: (() => void) | null = null;
  #restoreAfterConfigure = false;
  #disposed = false;

  /** Bind native renderer capabilities and the existing SugarCubes dialog service. */
  constructor(options: WorkflowMasonryHostLifecycleOptions) {
    this.#options = options;
    this.#renderers = new CubeSurfaceRendererLifecycle({
      document: options.document,
      logger: options.logger,
    });
    this.#controller = new WorkflowMasonryController({
      document: options.document,
      getContainer: () =>
        options.document.querySelector<HTMLElement>('.graph-canvas-panel') ??
        options.document.getElementById('graph-canvas-container'),
      isNodes2: () => this.#isNodes2(),
      getSections: () => {
        const graph = options.app?.canvas?.graph ?? options.app?.graph;
        return graph
          ? projectWorkflowMasonry(graph, options.getCubes(), options.getConnections(graph))
          : [];
      },
      getRenderer: () => this.#renderers.getRenderer(),
      getCardState: () => {
        const state = options.getCardState();
        if (!state) throw new Error('Cube card presentation is unavailable.');
        return state;
      },
      openCube: (cube) => this.#openCube(cube),
      alert: options.alert,
      reportError: options.reportError,
      onStateChange: () => this.#refreshButton(),
    });
    this.#toolbar = new WorkflowMasonryToolbarButton(options.document, () => this.toggle());
  }

  /** Register the persistent affordance and renderer transition subscription after host setup. */
  setup(): void {
    if (this.#unsubscribeRenderer || this.#disposed) return;
    ensureWorkflowMasonryStyles(this.#options.document);
    this.#unsubscribeRenderer = createComfyRendererModeChangeSource(this.#options.app).subscribe(
      () => {
        this.#controller.rendererChanged();
        if (!this.#isNodes2()) this.#renderers.release();
      },
    );
    this.#toolbar.setup();
    this.#refreshButton();
    this.#options.document.defaultView?.addEventListener('pagehide', this.#onPageHide);
  }

  /** Serve both the button and the registered Comfy command through the same use case. */
  toggle(): void {
    void this.#controller.toggle().catch((error: unknown) => this.#options.reportError(error));
  }

  /** Release native mounts before Comfy replaces a workflow's graph objects. */
  beforeConfigureGraph(): void {
    this.#restoreAfterConfigure = this.#controller.active;
    this.#controller.leaveImmediately();
    this.#releaseGraphObservers();
  }

  /** Restore the selected presentation against the newly configured workflow. */
  afterConfigureGraph(): void {
    this.#refreshButton();
    if (this.#restoreAfterConfigure && this.#isNodes2()) this.toggle();
    this.#restoreAfterConfigure = false;
  }

  /** Release all owned DOM, observers, subscriptions, renderer mounts, and animations. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#controller.leaveImmediately();
    this.#toolbar.dispose();
    this.#unsubscribeRenderer?.();
    this.#unsubscribeRenderer = null;
    this.#releaseGraphObservers();
    this.#renderers.dispose();
    this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
  }

  /** Tear down before the browser releases the Comfy Vue application. */
  readonly #onPageHide = (): void => this.dispose();

  /** Read the authoritative Comfy setting rather than inferring from stale graph DOM. */
  #isNodes2(): boolean {
    return resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
  }

  /** Reflect transient presentation and subscribe only while masonry needs graph updates. */
  #refreshButton(): void {
    const active = this.#controller.active;
    this.#toolbar.update(active, this.#isNodes2());
    if (active) this.#observeGraphs();
    else this.#releaseGraphObservers();
  }

  /** Observe topology through the existing reversible native graph adapter. */
  #observeGraphs(): void {
    this.#unsubscribeCards ??=
      this.#options.getCardState()?.subscribe(this.#onCardsChanged) ?? null;
    const graph = this.#options.app?.canvas?.graph ?? this.#options.app?.graph;
    const graphs: unknown[] = [graph, ...this.#options.getCubes().map((cube) => cube.subgraph)];
    for (const candidate of graphs) {
      if (!isObservableGraph(candidate) || this.#graphs.has(candidate)) continue;
      this.#graphs.set(
        candidate,
        new NativeSubgraphChangeObserver(candidate, () => {
          if (this.#disposed) return;
          void this.#controller
            .graphChanged()
            .catch((error: unknown) => this.#options.reportError(error));
        }),
      );
    }
  }

  /** Restore every host callback before changing renderer or graph ownership. */
  #releaseGraphObservers(): void {
    this.#unsubscribeCards?.();
    this.#unsubscribeCards = null;
    for (const observer of this.#graphs.values()) observer.dispose();
    this.#graphs.clear();
  }

  /** Refresh through the controller when shared card state changes in either presentation. */
  readonly #onCardsChanged = (): void => {
    if (this.#disposed) return;
    void this.#controller
      .graphChanged()
      .catch((error: unknown) => this.#options.reportError(error));
  };

  /** Enter the native Cube editor through Comfy's validated host API. */
  #openCube(cube: CubeNode): void {
    const canvas = this.#options.app?.canvas;
    const openSubgraph = canvas?.openSubgraph;
    if (typeof openSubgraph !== 'function') {
      this.#options.reportError(new Error('Comfy Cube editor navigation is unavailable.'));
      return;
    }
    openSubgraph.call(canvas, cube.subgraph, cube);
  }
}

/** Narrow dynamic host graph methods before passing them to the native observer adapter. */
function isObservableGraph(value: unknown): value is ObservableNativeSubgraph {
  return (
    isRecord(value) &&
    typeof value.add === 'function' &&
    typeof value.remove === 'function' &&
    (value.onAfterChange === undefined ||
      value.onAfterChange === null ||
      typeof value.onAfterChange === 'function')
  );
}
