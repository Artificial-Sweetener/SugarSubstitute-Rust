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
/** Coordinate workflow relation discovery and presentation across native host lifecycles. */
import type { ComfyApplication, ComfyGraph } from '../types/graph.js';
import { isRecord } from '../types/common.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import type { HostSurfaceChange } from '../core/HostSurfaceObserver.js';
import { NativeSubgraphChangeObserver } from '../surface/NativeSubgraphChangeObserver.js';
import { subscribeNativeNodeCardRefresh } from '../surface/NativeNodeCardRefresh.js';
import { WorkflowRelations } from './application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from './infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationExecution } from './infrastructure/NativeRelationExecution.js';
import { NativeRelationViews } from './infrastructure/NativeRelationViews.js';
import { NativeSharedSeedModes } from './infrastructure/NativeSharedSeedModes.js';
import { NativeWorkflowSeedReuse } from './infrastructure/NativeWorkflowSeedReuse.js';
import { NativeSeedMenuHost } from '../surface/appearance/NativeSeedMenuHost.js';
import type { FluentAppearanceSource } from '../surface/appearance/FluentAppearanceState.js';
import type { WorkflowSeedHistory } from '../execution/application/WorkflowSeedHistory.js';
import type { WorkflowSessionStore } from '../execution/application/WorkflowSessionStore.js';
import { NativeRelationWidgetRenderer } from './infrastructure/NativeRelationWidgetRenderer.js';
import { NativeRelationVisibility } from './infrastructure/NativeRelationVisibility.js';
import { NativeOverrideToolbarHost } from './infrastructure/NativeOverrideToolbarHost.js';
import { createNativeOverrideToolbarState } from './infrastructure/NativeOverrideToolbarState.js';
import { GlobalOverrideToolbar } from './presentation/GlobalOverrideToolbar.js';
import { CubeRelationPresenter } from './presentation/CubeRelationPresenter.js';
import { ensureRelationStyles } from './presentation/RelationStyles.js';
import type { CubeFaceCardStateService } from '../surface/CubeFaceCardStateService.js';
import type { WorkflowNodeConnection } from '../surface/WorkflowNodeOrder.js';

export interface RelationHostOptions {
  app: ComfyApplication | null;
  document: Document;
  logger: Pick<Console, 'warn'>;
  seeds: WorkflowSeedHistory;
  sessions: WorkflowSessionStore;
  appearance?(): FluentAppearanceSource;
  reportError(error: unknown): void;
  getCardState(): CubeFaceCardStateService | null;
  getConnections?(graph: ComfyGraph): readonly WorkflowNodeConnection[];
}

/** Own relation lifecycle while delegating policy, native interaction, persistence, and rendering. */
export class RelationHostLifecycle {
  readonly #port: NativeRelationWorkflowPort;
  readonly relations: WorkflowRelations;
  readonly seedReuse: NativeWorkflowSeedReuse;
  readonly #seedMenus: NativeSeedMenuHost;
  readonly #execution: NativeRelationExecution;
  readonly #toolbar: GlobalOverrideToolbar;
  readonly #cards: CubeRelationPresenter;
  readonly #views: NativeRelationViews;
  readonly #visibility = new NativeRelationVisibility();
  readonly #seedModes = new NativeSharedSeedModes();
  readonly #graphs = new Map<ComfyGraph, NativeSubgraphChangeObserver>();
  #presence: HostSurfaceObserver | null = null;
  #chromePresence: HostSurfaceObserver | null = null;
  #pending = false;
  #valuesPending = false;
  #active = false;
  #rendering = false;
  #configuring = false;
  #generation = 0;
  #unsubscribeChoices: (() => void) | null = null;
  #presentationFrame: number | null = null;

  /** Compose explicit collaborators against the root workflow rather than the navigated canvas graph. */
  constructor(private readonly options: RelationHostOptions) {
    this.#port = new NativeRelationWorkflowPort(
      () => options.app?.graph,
      options.document,
      options.logger,
      options.getCardState,
      options.getConnections,
    );
    this.relations = new WorkflowRelations(this.#port);
    this.seedReuse = new NativeWorkflowSeedReuse(
      options.app,
      options.sessions,
      options.seeds,
      this.relations,
      this.#port.inventorySource,
      options.document,
      options.reportError,
    );
    this.#seedMenus = new NativeSeedMenuHost(
      options.app,
      options.document,
      (node, name) => this.seedReuse.field(node, name),
      options.reportError,
    );
    this.#execution = new NativeRelationExecution(
      this.relations,
      this.#port.inventorySource,
      options.logger,
      () => options.app?.graph?.serialize?.(),
    );
    const widgets = new NativeRelationWidgetRenderer(
      options.document,
      options.reportError,
      (key) => this.seedReuse.group(key),
      options.appearance,
    );
    this.#views = new NativeRelationViews(options.document);
    this.#toolbar = new GlobalOverrideToolbar(
      options.document,
      this.relations,
      options.reportError,
      widgets,
      new NativeOverrideToolbarHost(options.document),
      createNativeOverrideToolbarState(options.document, options.logger),
    );
    this.#cards = new CubeRelationPresenter(
      options.document,
      this.relations,
      () => {
        const nodes = new Map(
          [...this.#port.inventorySource.owners.values()].map((owner) => [owner.nodeId, owner]),
        );
        return [...nodes].flatMap(([nodeId, owner]) =>
          nodeId
            ? this.#views.read(owner.face, owner.cubeId).map((surface) => ({
                nodeId,
                surface,
              }))
            : [],
        );
      },
      options.reportError,
      widgets,
    );
    this.relations.subscribe(() => this.#present());
  }

  /** Attach after Comfy initializes its root graph and shell. */
  setup(): void {
    if (this.#active) return;
    this.#active = true;
    this.#seedMenus.setup();
    ensureRelationStyles(this.options.document);
    const changed = (changes: readonly HostSurfaceChange[]): void => {
      this.#views.invalidate(changes.map((change) => change.element));
      if (!this.#rendering) this.#schedulePresentation();
    };
    this.#presence = new HostSurfaceObserver(
      this.options.document.body,
      '.sugarcubes-cube-face__node-card,.lg-node[data-node-id],[data-sugarcubes-cube-face-projection]',
      changed,
      { content: true, exclude: '.sc-field-relation,.sc-overrides-toolbar' },
    );
    this.#chromePresence = new HostSurfaceObserver(
      this.options.document.body,
      '.actionbar-container',
      changed,
    );
    this.options.document.addEventListener('input', this.#nativeEdit, true);
    this.options.document.addEventListener('change', this.#nativeEdit, true);
    this.#unsubscribeChoices = subscribeNativeNodeCardRefresh(this.options.document, [], () =>
      this.#schedule(),
    );
    this.#schedule();
  }

  /** Drop stale mounted views before another native workflow is configured. */
  beforeConfigureGraph(): void {
    this.#configuring = true;
    this.#seedMenus.close();
    this.#generation++;
    if (this.#presentationFrame !== null)
      this.options.document.defaultView?.cancelAnimationFrame(this.#presentationFrame);
    this.#presentationFrame = null;
    this.#pending = false;
    this.#valuesPending = false;
    for (const observer of this.#graphs.values()) observer.dispose();
    this.#graphs.clear();
    this.#cards.dispose();
    this.#views.clear();
    this.#visibility.dispose();
    this.#seedModes.dispose();
    this.relations.invalidate();
  }

  /** Discover loaded instances without relying on whether their native cards have mounted. */
  afterConfigureGraph(): void {
    this.#configuring = false;
    if (!this.#active) return;
    try {
      this.#observeGraphs();
      this.#port.joinTransaction(() => this.relations.reconcile());
    } catch (error: unknown) {
      this.options.reportError(error);
    }
  }

  /** Reconcile reordered membership inside the graph mutation owner's native history transaction. */
  reconcileCubeOrder(): void {
    this.#port.joinTransaction(() => this.relations.reconcile());
  }

  /** Finish identified topology and value consequences inside the native mutation's history entry. */
  readonly commitNativeTransaction = (): void => {
    if (!this.#active || this.#configuring || this.#port.editing) return;
    for (const observer of this.#graphs.values()) observer.flush();
    const discover = this.#pending;
    if (!discover && !this.#valuesPending) return;
    this.#pending = false;
    this.#valuesPending = false;
    this.#port.joinTransaction(() => {
      if (discover) this.#observeGraphs();
      this.relations.reconcile(discover);
    });
  };

  /** Prepare either queue path through the same workflow relation owner. */
  prepare(payload: unknown): unknown {
    return this.#execution.prepare(payload);
  }

  /** Present the authoritative override menu from an alternate frame affordance. */
  openOverrideMenu(trigger: HTMLElement): void {
    this.#toolbar.openMenu(trigger);
  }

  /** Delegate temporary embedding layout to the toolbar's presentation and preference owners. */
  setOverridesInline(inline: boolean): void {
    this.#toolbar.setPresentation(inline ? 'substitute' : 'comfy');
  }

  /** Suspend workflow-wide override presentation while retaining authoritative relation state. */
  setEditorActive(active: boolean): void {
    this.#toolbar.setSuspended(active);
  }

  /** Release all reversible host hooks and presentation at application teardown. */
  dispose(): void {
    this.#active = false;
    this.#seedMenus.dispose();
    this.#presence?.disconnect();
    this.#chromePresence?.disconnect();
    this.#unsubscribeChoices?.();
    this.#unsubscribeChoices = null;
    this.beforeConfigureGraph();
    this.#toolbar.dispose();
    this.options.document.removeEventListener('input', this.#nativeEdit, true);
    this.options.document.removeEventListener('change', this.#nativeEdit, true);
  }

  /** Coalesce native edits after their own promoted-widget forwarding callbacks. */
  readonly #nativeEdit = (event: Event): void => {
    if (!this.#active || this.#configuring) return;
    if (!(event.target instanceof Element)) return;
    if (!event.target.closest('.lg-node,.dom-widget,[data-cube-node-id],[data-masonry-card]'))
      return;
    if (event.target.closest('[role="listbox"],.sc-field-relation,.sc-model-popup')) return;
    this.#scheduleValues();
  };

  /** Coalesce native value delivery without rebuilding validated topology bindings. */
  #scheduleValues(): void {
    if (!this.#active || this.#configuring) return;
    if (this.#valuesPending) return;
    this.#valuesPending = true;
    const generation = this.#generation;
    queueMicrotask(() =>
      queueMicrotask(() => {
        if (
          !this.#active ||
          this.#configuring ||
          generation !== this.#generation ||
          !this.#valuesPending
        )
          return;
        this.#valuesPending = false;
        try {
          this.relations.reconcile(false);
        } catch (error: unknown) {
          this.options.reportError(error);
        }
      }),
    );
  }

  /** Observe topology changes independently from the currently active graph or masonry surface. */
  #observeGraphs(): void {
    const active = new Set<ComfyGraph>();
    const visit = (graph: ComfyGraph | null | undefined): void => {
      if (!graph || active.has(graph)) return;
      active.add(graph);
      for (const node of graph._nodes ?? graph.nodes ?? [])
        if (isRecord(node.subgraph)) visit(node.subgraph);
      if (
        !this.#graphs.has(graph) &&
        typeof graph.add === 'function' &&
        typeof graph.remove === 'function'
      )
        this.#graphs.set(
          graph,
          new NativeSubgraphChangeObserver(
            graph,
            (changes) => {
              if (changes.some((change) => change.kind !== 'change')) this.#schedule();
              else this.#scheduleValues();
            },
            (kind) => kind !== 'change' || !this.#port.editing,
          ),
        );
    };
    visit(this.options.app?.graph);
    for (const [graph, observer] of this.#graphs)
      if (!active.has(graph)) {
        observer.dispose();
        this.#graphs.delete(graph);
      }
  }

  /** Reconcile once per native event batch and report failures through SugarCubes feedback. */
  #schedule(): void {
    if (!this.#active || this.#configuring || this.#pending) return;
    this.#pending = true;
    const generation = this.#generation;
    queueMicrotask(() => {
      if (!this.#active || this.#configuring || generation !== this.#generation || !this.#pending)
        return;
      this.#pending = false;
      try {
        this.#observeGraphs();
        this.relations.reconcile();
      } catch (error: unknown) {
        this.options.reportError(error);
      }
    });
  }

  /** Render from a completed application snapshot without re-entering candidate discovery. */
  #present(): void {
    if (!this.#active || this.#configuring) return;
    this.#rendering = true;
    try {
      const snapshot = this.relations.snapshot();
      if (this.#seedModes.reconcile(snapshot, this.#port.inventorySource))
        this.#port.save(snapshot.state);
      const changed = this.#visibility.reconcile(snapshot, this.#port.inventorySource);
      if (changed.length) this.#port.refresh(changed);
      this.#schedulePresentation();
    } finally {
      this.#presence?.discardPending();
      this.#chromePresence?.discardPending();
      this.#rendering = false;
    }
  }

  /** Project settled native surfaces once per frame while keeping history and effective values immediate. */
  #schedulePresentation(): void {
    if (!this.#active || this.#configuring || this.#presentationFrame !== null) return;
    const generation = this.#generation;
    const render = (): void => {
      if (generation !== this.#generation) return;
      this.#presentationFrame = null;
      if (!this.#active || this.#configuring) return;
      this.#rendering = true;
      try {
        this.#toolbar.reconcile();
        this.#cards.reconcile();
      } catch (error: unknown) {
        this.options.reportError(error);
      } finally {
        this.#presence?.discardPending();
        this.#chromePresence?.discardPending();
        this.#rendering = false;
      }
    };
    const windowRef = this.options.document.defaultView;
    if (windowRef) this.#presentationFrame = windowRef.requestAnimationFrame(render);
    else {
      this.#presentationFrame = -1;
      queueMicrotask(render);
    }
  }
}
