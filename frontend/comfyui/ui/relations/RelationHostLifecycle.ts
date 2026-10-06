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
import { NativeSubgraphChangeObserver } from '../surface/NativeSubgraphChangeObserver.js';
import { subscribeNativeNodeCardRefresh } from '../surface/NativeNodeCardRefresh.js';
import { WorkflowRelations } from './application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from './infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationExecution } from './infrastructure/NativeRelationExecution.js';
import { NativeRelationViews } from './infrastructure/NativeRelationViews.js';
import { NativeSharedSeedModes } from './infrastructure/NativeSharedSeedModes.js';
import { NativeRelationWidgetRenderer } from './infrastructure/NativeRelationWidgetRenderer.js';
import { NativeRelationVisibility } from './infrastructure/NativeRelationVisibility.js';
import { attachNativeOverrideToolbar } from './infrastructure/NativeOverrideToolbarHost.js';
import { GlobalOverrideToolbar } from './presentation/GlobalOverrideToolbar.js';
import { CubeRelationPresenter } from './presentation/CubeRelationPresenter.js';
import { ensureRelationStyles } from './presentation/RelationStyles.js';
import type { CubeFaceCardStateService } from '../surface/CubeFaceCardStateService.js';
import type { WorkflowNodeConnection } from '../surface/WorkflowNodeOrder.js';

export interface RelationHostOptions {
  app: ComfyApplication | null;
  document: Document;
  logger: Pick<Console, 'warn'>;
  reportError(error: unknown): void;
  getCardState(): CubeFaceCardStateService | null;
  getConnections?(graph: ComfyGraph): readonly WorkflowNodeConnection[];
}

/** Own relation lifecycle while delegating policy, native interaction, persistence, and rendering. */
export class RelationHostLifecycle {
  readonly #port: NativeRelationWorkflowPort;
  readonly relations: WorkflowRelations;
  readonly #execution: NativeRelationExecution;
  readonly #toolbar: GlobalOverrideToolbar;
  readonly #cards: CubeRelationPresenter;
  readonly #views: NativeRelationViews;
  readonly #visibility = new NativeRelationVisibility();
  readonly #seedModes = new NativeSharedSeedModes();
  readonly #graphs = new Map<ComfyGraph, NativeSubgraphChangeObserver>();
  #presence: HostSurfaceObserver | null = null;
  #pending = false;
  #valuesPending = false;
  #active = false;
  #rendering = false;
  #unsubscribeChoices: (() => void) | null = null;

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
    this.#execution = new NativeRelationExecution(
      this.relations,
      this.#port.inventorySource,
      options.logger,
      () => options.app?.graph?.serialize?.(),
    );
    const widgets = new NativeRelationWidgetRenderer(options.document, options.reportError);
    this.#views = new NativeRelationViews(options.document);
    this.#toolbar = new GlobalOverrideToolbar(
      options.document,
      this.relations,
      options.reportError,
      widgets,
      (element) => attachNativeOverrideToolbar(options.document, element),
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
    ensureRelationStyles(this.options.document);
    this.#presence = new HostSurfaceObserver(
      this.options.document.body,
      '.sugarcubes-cube-face__node-card,.lg-node[data-node-id],[data-sugarcubes-cube-face-projection],.actionbar-container',
      (changes) => {
        this.#views.invalidate(changes.map((change) => change.element));
        if (!this.#rendering) this.#present();
      },
      { content: true, exclude: '.sc-field-relation,.sc-overrides-toolbar' },
    );
    this.options.document.addEventListener('input', this.#nativeEdit, true);
    this.options.document.addEventListener('change', this.#nativeEdit, true);
    this.#unsubscribeChoices = subscribeNativeNodeCardRefresh(this.options.document, [], () =>
      this.#schedule(),
    );
    this.afterConfigureGraph();
  }

  /** Drop stale mounted views before another native workflow is configured. */
  beforeConfigureGraph(): void {
    for (const observer of this.#graphs.values()) observer.dispose();
    this.#graphs.clear();
    this.#cards.dispose();
    this.#views.clear();
    this.#visibility.dispose();
    this.#seedModes.dispose();
  }

  /** Discover loaded instances without relying on whether their native cards have mounted. */
  afterConfigureGraph(): void {
    this.#schedule();
  }

  /** Prepare either queue path through the same workflow relation owner. */
  prepare(payload: unknown): unknown {
    return this.#execution.prepare(payload);
  }

  /** Release all reversible host hooks and presentation at application teardown. */
  dispose(): void {
    this.#active = false;
    this.#presence?.disconnect();
    this.#unsubscribeChoices?.();
    this.#unsubscribeChoices = null;
    this.beforeConfigureGraph();
    this.#toolbar.dispose();
    this.options.document.removeEventListener('input', this.#nativeEdit, true);
    this.options.document.removeEventListener('change', this.#nativeEdit, true);
  }

  /** Coalesce native edits after their own promoted-widget forwarding callbacks. */
  readonly #nativeEdit = (event: Event): void => {
    if (!(event.target instanceof Element)) return;
    if (!event.target.closest('.lg-node,.dom-widget,[data-cube-node-id],[data-masonry-card]'))
      return;
    if (event.target.closest('[role="listbox"],.sc-field-relation,.sc-model-popup')) return;
    if (this.#valuesPending) return;
    this.#valuesPending = true;
    queueMicrotask(() =>
      queueMicrotask(() => {
        this.#valuesPending = false;
        if (!this.#active) return;
        try {
          this.relations.reconcile(false);
        } catch (error: unknown) {
          this.options.reportError(error);
        }
      }),
    );
  };

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
            () => this.#schedule(),
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
    if (!this.#active || this.#pending) return;
    this.#pending = true;
    queueMicrotask(() => {
      this.#pending = false;
      if (!this.#active) return;
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
    if (!this.#active) return;
    this.#rendering = true;
    try {
      const snapshot = this.relations.snapshot();
      if (this.#seedModes.reconcile(snapshot, this.#port.inventorySource))
        this.#port.save(snapshot.state);
      const changed = this.#visibility.reconcile(snapshot, this.#port.inventorySource);
      if (changed.length) this.#port.refresh(changed);
      this.#toolbar.reconcile();
      this.#cards.reconcile();
    } finally {
      this.#presence?.discardPending();
      this.#rendering = false;
    }
  }
}
