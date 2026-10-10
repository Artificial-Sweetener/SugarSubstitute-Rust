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
/** Keep root Cube identity and presentation aligned with Comfy's workflow graph. */

import {
  NativeSubgraphChangeObserver,
  type ObservableNativeSubgraph,
} from '../../surface/NativeSubgraphChangeObserver.js';
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import { labelEmptyCubeBoundaryAffordances } from '../geometry/NativeCubeBoundaryLayout.js';
import {
  isCubeNode,
  isDraftCubeNode,
  requireCubeIdentity,
  type CubeNode,
} from './ComfyCubeNodeFactory.js';
import { CubeNodeCatalog, readInstanceId } from './CubeNodeCatalog.js';
import type { CubeGraphInventory } from './CubeGraphInventory.js';
import type { CubeWorkflowNodePackMetadata } from './CubeWorkflowNodePackMetadata.js';
import { NativeCubeDefinitionOwnership } from '../graph/NativeCubeDefinitionOwnership.js';

export interface CubeNodeLifecycleGraph extends ObservableNativeSubgraph {
  _nodes?: unknown[];
  onConfigure?: ((data: UnknownRecord) => void) | null;
}

export interface ComfyCubeNodeLifecycleAdapterOptions {
  graph: CubeNodeLifecycleGraph;
  inventory: CubeGraphInventory;
  catalog: CubeNodeCatalog;
  events: EventTarget;
  createInstanceId(): string;
  discardDefinition(id: string): void;
  nodePackMetadata: CubeWorkflowNodePackMetadata;
  logger: Pick<Console, 'debug' | 'error'>;
}

/** Own host lifecycle hooks, pasted identity, and the non-authoritative Cube index. */
export class ComfyCubeNodeLifecycleAdapter {
  readonly #graph: CubeNodeLifecycleGraph;
  readonly #catalog: CubeNodeCatalog;
  readonly #inventory: CubeGraphInventory;
  readonly #events: EventTarget;
  readonly #createInstanceId: () => string;
  readonly #logger: Pick<Console, 'debug' | 'error'>;
  readonly #nodePackMetadata: CubeWorkflowNodePackMetadata;
  readonly #unsubscribeNodePackMetadata: () => void;
  readonly #previousConfigure: ((data: UnknownRecord) => void) | null;
  readonly #configureHook: (data: UnknownRecord) => void;
  readonly #topology: NativeSubgraphChangeObserver;
  readonly #definitions: NativeCubeDefinitionOwnership;
  #disposed = false;

  /** Observe completed topology changes without capturing native renderer callbacks. */
  constructor(options: ComfyCubeNodeLifecycleAdapterOptions) {
    this.#graph = options.graph;
    this.#catalog = options.catalog;
    this.#inventory = options.inventory;
    this.#events = options.events;
    this.#createInstanceId = options.createInstanceId;
    this.#logger = options.logger;
    this.#nodePackMetadata = options.nodePackMetadata;
    this.#definitions = new NativeCubeDefinitionOwnership(
      options.graph,
      options.createInstanceId,
      options.discardDefinition,
    );
    this.#previousConfigure = options.graph.onConfigure ?? null;
    this.#configureHook = (data) => {
      this.#previousConfigure?.call(this.#graph, data);
      this.#reconcile();
    };
    options.graph.onConfigure = this.#configureHook;
    this.#topology = new NativeSubgraphChangeObserver(
      options.graph,
      () => this.#reconcile(),
      (kind) => kind === 'add' || kind === 'remove',
    );
    options.events.addEventListener('litegraph:canvas', this.#handleCanvasChange);
    this.#unsubscribeNodePackMetadata = options.nodePackMetadata.subscribe(() => this.#reconcile());
    this.#reconcile();
  }

  /** Restore callbacks when the graph-bound runtime is replaced. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#topology.dispose();
    if (this.#graph.onConfigure === this.#configureHook) {
      this.#graph.onConfigure = this.#previousConfigure;
    }
    this.#events.removeEventListener('litegraph:canvas', this.#handleCanvasChange);
    this.#unsubscribeNodePackMetadata();
  }

  /** Reconcile only after Comfy has configured all nodes created by one canvas operation. */
  readonly #handleCanvasChange = (event: Event): void => {
    if (!isRecord(event) || !isRecord(event.detail) || event.detail.subType !== 'after-change') {
      return;
    }
    try {
      this.#reconcile();
    } catch (error: unknown) {
      this.#logger.error('SugarCubes could not reconcile Cube nodes after a graph change.', {
        error,
      });
      throw error;
    }
  };

  /** Assign fresh per-instance identities to copied Cubes before rebuilding the index. */
  #reconcile(): void {
    if (this.#disposed) return;
    const seen = new Map<string, CubeNode>();
    const values = this.#inventory.snapshot().rootCubes;
    for (const value of values) {
      if (!isCubeNode(value)) continue;
      this.#nodePackMetadata.apply(value);
      if (isDraftCubeNode(value)) labelEmptyCubeBoundaryAffordances(value.subgraph);
      let instanceId = readInstanceId(value);
      const existing = seen.get(instanceId);
      if (existing && existing !== value) {
        instanceId = this.#assignFreshInstanceId(value, seen);
      }
      seen.set(instanceId, value);
    }
    this.#definitions.reconcile([...seen.values()]);
    this.#catalog.replace(values);
  }

  /** Replace one copied identity without changing its Cube definition identity. */
  #assignFreshInstanceId(node: CubeNode, reserved: ReadonlyMap<string, CubeNode>): string {
    for (let attempt = 0; attempt < 32; attempt += 1) {
      const candidate = this.#createInstanceId().trim();
      if (!candidate || reserved.has(candidate) || this.#catalog.get(candidate)) continue;
      const prior = readInstanceId(node);
      requireCubeIdentity(node).instance_id = candidate;
      this.#logger.debug('SugarCubes assigned a fresh identity to a copied Cube node.', {
        node_id: String(node.id),
        prior_instance_id: prior,
        instance_id: candidate,
      });
      return candidate;
    }
    throw new Error(`Could not allocate a unique identity for copied Cube '${String(node.id)}'.`);
  }
}
