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
/** Bind reusable loader relationships to live Comfy topology independently of card and preview lifetimes. */
import type { ComfyGraph, ComfyNode } from '../../types/graph.js';
import { InputAssetAssociations } from '../application/InputAssetAssociations.js';
import type {
  InputImageAssociation,
  InputEditorBinding,
  InputAssetBindingLookup,
} from '../application/InputEditorPorts.js';
import type { InputAssetAssociationGraph } from '../domain/InputAssetAssociation.js';
import type { ComfyInputBindings } from './ComfyInputBindings.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { ComfyInputAssociationGraph } from './ComfyInputAssociationGraph.js';
import { ComfyInputAssociationObserver } from './ComfyInputAssociationObserver.js';
import { readInputBinding } from './InputBindingMetadata.js';
import type { SpatialTopology } from '../domain/CanvasDimensionAuthority.js';

/** Own lifecycle and host translation while application policy owns every relationship decision. */
export class ComfyInputAssociations implements InputAssetBindingLookup {
  readonly relationships = new InputAssetAssociations();
  readonly #graph: ComfyInputAssociationGraph;
  readonly #observers = new Map<ComfyGraph, ComfyInputAssociationObserver>();
  readonly #facts = new Map<ComfyGraph, InputAssetAssociationGraph>();
  readonly #dirty = new Set<ComfyGraph>();
  readonly #spatial = new Map<ComfyGraph, SpatialTopology>();
  readonly #topologyListeners = new Set<() => void>();
  #membership = true;
  #scheduled = false;
  #active = false;
  #generation = 0;
  #membershipReads = 0;
  #topologyReads = 0;

  /** Inject the root workflow and authoritative source-binding registry instead of reading rendered cards. */
  constructor(
    private readonly loaders: ComfyInputLoaders,
    private readonly bindings: ComfyInputBindings,
    private readonly root: () => ComfyGraph | null | undefined,
    private readonly report: (error: unknown) => void,
    logger: Pick<Console, 'warn'>,
  ) {
    this.#graph = new ComfyInputAssociationGraph(loaders, logger);
  }

  /** Report whether a configured workflow is being observed; false while a workflow loads. */
  get active(): boolean {
    return this.#active;
  }

  /** Expose the field inventory and flattened workflow paths read at the last settled flush. */
  get inventory(): Pick<
    ComfyInputAssociationGraph,
    'fields' | 'workflowKey' | 'path' | 'node' | 'kind'
  > {
    return this.#graph;
  }

  /** Merge every observed scope's spatial connections from the last settled flush. */
  topology(): SpatialTopology {
    const scopes = [...this.#spatial.values()];
    return {
      nodes: scopes.flatMap((scope) => scope.nodes),
      edges: scopes.flatMap((scope) => scope.edges),
    };
  }

  /** Observe every settled topology flush, including changes that leave associations unchanged. */
  subscribeTopology(listener: () => void): () => void {
    this.#topologyListeners.add(listener);
    return () => {
      this.#topologyListeners.delete(listener);
    };
  }

  /** Expose the same opaque field identity to future application consumers. */
  key(node: ComfyNode, field: string): string {
    return this.#graph.key(node, field);
  }

  /** Resolve any owned endpoint through the authoritative binding registry for reusable application use cases. */
  binding(endpoint: string): InputEditorBinding | null {
    const source = this.#graph.field(endpoint);
    return source ? this.bindings.get(source.node, source.loader) : null;
  }

  /** Adapt a relationship query to the existing source owner without introducing another source picker. */
  image(node: ComfyNode, field: string): InputImageAssociation {
    const key = this.key(node, field);
    return {
      read: () => {
        const association = this.relationships.read(key);
        if (association.kind !== 'associated') return null;
        const source = this.binding(association.image);
        return source?.loader.kind === 'image' ? source : null;
      },
      editable: () => {
        const image = this.relationships.exclusiveImage(key);
        if (!image) return false;
        return this.#editablePair(key, image);
      },
      subscribe: (listener) =>
        this.relationships.subscribe((changed) => {
          if (changed.has(key)) listener();
        }),
    };
  }

  /** Capture settled queue-time topology without discovering candidates from value or presentation changes. */
  preparationPairs(): ReadonlyMap<string, string> {
    if (!this.#active) return new Map();
    if (this.#scheduled || this.#membership || this.#dirty.size) this.#flush();
    const result = new Map<string, string>();
    for (const [mask, image] of this.relationships.exclusivePairs()) {
      const maskKey = this.#graph.workflowKey(mask),
        imageKey = this.#graph.workflowKey(image);
      if (maskKey && imageKey && this.#editablePair(mask, image)) result.set(maskKey, imageKey);
    }
    return result;
  }

  /** Require one scalar decoded mask and a positioned canvas before exposing or preparing edits. */
  #editablePair(mask: string, image: string): boolean {
    const field = this.#graph.field(mask),
      source = this.binding(image);
    const state = field
      ? readInputBinding(field.node.properties, 'mask', field.loader.field)
      : null;
    return Boolean(
      field &&
        state &&
        source &&
        !field.loader.ordered &&
        !source.loader.ordered &&
        state.entries.length === 1 &&
        source.state.entries.length === 1 &&
        source.context?.supported &&
        source.context.document &&
        source.canEditMask &&
        this.loaders
          .assetPorts()
          .get(field.loader.classType)
          ?.outputs.filter((kind) => kind === 'mask').length === 1,
    );
  }

  /** Activate the current native workflow after setup or an explicitly completed configuration. */
  start(): void {
    this.#active = true;
    this.#membership = true;
    this.#flush();
  }

  /** Re-read membership only after Comfy refreshes its node-definition catalog. */
  definitionsChanged(): void {
    if (this.#active) {
      this.#membership = true;
      this.#schedule();
    }
  }

  /** Retire callbacks and relationship identities before another workflow replaces live native handles. */
  beforeConfigureGraph(): void {
    this.#active = false;
    this.#generation++;
    this.#scheduled = false;
    for (const observer of this.#observers.values()) observer.dispose();
    this.#observers.clear();
    this.#facts.clear();
    this.#spatial.clear();
    this.#dirty.clear();
    this.#graph.reconcile(null);
    this.relationships.replace([]);
    this.#topologyChanged();
  }

  /** Release all graph ownership when the browser host is retired. */
  dispose(): void {
    this.beforeConfigureGraph();
  }

  /** Report native discovery work separately from source, value and popup activity. */
  diagnostics() {
    return {
      graphs: this.#observers.size,
      membershipReads: this.#membershipReads,
      topologyReads: this.#topologyReads,
    };
  }

  /** Coalesce synchronous native connection callbacks until their graph transaction is settled. */
  #schedule(): void {
    if (!this.#active || this.#scheduled) return;
    this.#scheduled = true;
    const generation = this.#generation;
    queueMicrotask(() => {
      if (this.#active && generation === this.#generation) this.#flush();
    });
  }

  /** Refresh changed scopes and their exposed ancestors while leaving unrelated graph branches cached. */
  #flush(): void {
    this.#scheduled = false;
    if (!this.#active) return;
    try {
      const dirty = new Set<ComfyGraph>();
      if (this.#membership) {
        const scopes = this.#graph.reconcile(this.root());
        this.#membershipReads++;
        const graphs = new Set(scopes.map((scope) => scope.graph));
        for (const [graph, observer] of this.#observers)
          if (!graphs.has(graph)) {
            observer.dispose();
            this.#observers.delete(graph);
          }
        this.#facts.clear();
        this.#spatial.clear();
        for (const { graph } of scopes) {
          if (!this.#observers.has(graph))
            this.#observers.set(
              graph,
              new ComfyInputAssociationObserver(graph, (membership) => {
                if (!this.#active) return;
                this.#membership ||= membership;
                this.#dirty.add(graph);
                this.#schedule();
              }),
            );
          else this.#observers.get(graph)?.reconcile();
          dirty.add(graph);
        }
      } else
        for (const graph of this.#dirty) {
          let scope = this.#graph.scope(graph);
          while (scope && !dirty.has(scope.graph)) {
            dirty.add(scope.graph);
            scope = scope.parent ? this.#graph.scope(scope.parent) : null;
          }
        }
      this.#membership = false;
      this.#dirty.clear();
      for (const graph of dirty) {
        this.#facts.set(graph, this.#graph.read(graph));
        this.#spatial.set(graph, this.#graph.spatial(graph));
        this.#topologyReads++;
      }
      this.relationships.replace([...this.#facts.values()]);
    } catch (error: unknown) {
      this.#spatial.clear();
      this.relationships.replace([]);
      this.report(error);
    }
    this.#topologyChanged();
  }

  /** Notify topology observers, isolating their failures. */
  #topologyChanged(): void {
    for (const listener of this.#topologyListeners)
      try {
        listener();
      } catch (error: unknown) {
        this.report(error);
      }
  }
}
