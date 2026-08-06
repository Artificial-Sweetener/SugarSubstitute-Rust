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
/** Keep Cube identity and presentation aligned across Comfy's complete graph registry. */

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

export interface CubeNodeLifecycleGraph {
  _nodes?: unknown[];
  subgraphs?: ReadonlyMap<string, { _nodes?: unknown[] }>;
  onConfigure?: ((data: UnknownRecord) => void) | null;
  onNodeAdded?: ((node: unknown) => void) | null;
  onNodeRemoved?: ((node: unknown) => void) | null;
}

export interface ComfyCubeNodeLifecycleAdapterOptions {
  graph: CubeNodeLifecycleGraph;
  catalog: CubeNodeCatalog;
  events: EventTarget;
  createInstanceId(): string;
  logger: Pick<Console, 'debug' | 'error'>;
}

/** Own host lifecycle hooks, pasted identity, and the non-authoritative Cube index. */
export class ComfyCubeNodeLifecycleAdapter {
  readonly #graph: CubeNodeLifecycleGraph;
  readonly #catalog: CubeNodeCatalog;
  readonly #events: EventTarget;
  readonly #createInstanceId: () => string;
  readonly #logger: Pick<Console, 'debug' | 'error'>;
  readonly #previousConfigure: ((data: UnknownRecord) => void) | null;
  readonly #previousNodeAdded: ((node: unknown) => void) | null;
  readonly #previousNodeRemoved: ((node: unknown) => void) | null;
  readonly #configureHook: (data: UnknownRecord) => void;
  readonly #nodeAddedHook: (node: unknown) => void;
  readonly #nodeRemovedHook: (node: unknown) => void;

  /** Install graph and completed-canvas-change synchronization around prior callbacks. */
  constructor(options: ComfyCubeNodeLifecycleAdapterOptions) {
    this.#graph = options.graph;
    this.#catalog = options.catalog;
    this.#events = options.events;
    this.#createInstanceId = options.createInstanceId;
    this.#logger = options.logger;
    this.#previousConfigure = options.graph.onConfigure ?? null;
    this.#previousNodeAdded = options.graph.onNodeAdded ?? null;
    this.#previousNodeRemoved = options.graph.onNodeRemoved ?? null;
    this.#configureHook = (data) => {
      this.#previousConfigure?.call(this.#graph, data);
      this.#reconcile();
    };
    this.#nodeAddedHook = (node) => {
      this.#previousNodeAdded?.call(this.#graph, node);
      if (isCubeNode(node)) this.#reconcile();
    };
    this.#nodeRemovedHook = (node) => {
      this.#previousNodeRemoved?.call(this.#graph, node);
      if (isCubeNode(node) && this.#catalog.get(readInstanceId(node)) === node) {
        this.#catalog.remove(readInstanceId(node));
      }
    };
    options.graph.onConfigure = this.#configureHook;
    options.graph.onNodeAdded = this.#nodeAddedHook;
    options.graph.onNodeRemoved = this.#nodeRemovedHook;
    options.events.addEventListener('litegraph:canvas', this.#handleCanvasChange);
    this.#reconcile();
  }

  /** Restore callbacks when the graph-bound runtime is replaced. */
  dispose(): void {
    if (this.#graph.onConfigure === this.#configureHook) {
      this.#graph.onConfigure = this.#previousConfigure;
    }
    if (this.#graph.onNodeAdded === this.#nodeAddedHook) {
      this.#graph.onNodeAdded = this.#previousNodeAdded;
    }
    if (this.#graph.onNodeRemoved === this.#nodeRemovedHook) {
      this.#graph.onNodeRemoved = this.#previousNodeRemoved;
    }
    this.#events.removeEventListener('litegraph:canvas', this.#handleCanvasChange);
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
    const seen = new Map<string, CubeNode>();
    const values = collectGraphNodes(this.#graph);
    for (const value of values) {
      if (!isCubeNode(value)) continue;
      if (isDraftCubeNode(value)) labelEmptyCubeBoundaryAffordances(value.subgraph);
      let instanceId = readInstanceId(value);
      const existing = seen.get(instanceId);
      if (existing && existing !== value) {
        instanceId = this.#assignFreshInstanceId(value, seen);
      }
      seen.set(instanceId, value);
    }
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

/** Collect root and native-subgraph nodes exactly once from Comfy's global definition map. */
function collectGraphNodes(graph: CubeNodeLifecycleGraph): unknown[] {
  const values = [...(graph._nodes ?? [])];
  for (const subgraph of graph.subgraphs?.values() ?? []) {
    values.push(...(subgraph._nodes ?? []));
  }
  return values;
}
