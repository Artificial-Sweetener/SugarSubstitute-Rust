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
/** Observe identified native connection, membership and configure lifecycles without value or DOM discovery. */
import type { ComfyGraph, ComfyNode } from '../../types/graph.js';
import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
import { getGraphNodes } from '../../graph/GraphQuery.js';

/** Keep one graph's reversible native callback leases with its association lifecycle. */
export class ComfyInputAssociationObserver {
  readonly #membership: NativeSubgraphChangeObserver | null;
  readonly #nodes = new Map<ComfyNode, () => void>();
  readonly #stopConfigure: () => void;
  #active = true;

  /** Subscribe only to actual native topology operations and preserve their existing callbacks and receivers. */
  constructor(
    private readonly graph: ComfyGraph,
    private readonly changed: (membership: boolean) => void,
  ) {
    this.#membership =
      typeof graph.add === 'function' && typeof graph.remove === 'function'
        ? new NativeSubgraphChangeObserver(
            graph,
            () => {
              if (this.#active) changed(true);
            },
            (kind) => kind !== 'change',
          )
        : null;
    const configure = graph.configure;
    if (typeof configure === 'function') {
      const configured = (...args: unknown[]): unknown => {
        const result: unknown = configure.apply(graph, args);
        if (this.#active) changed(true);
        return result;
      };
      graph.configure = configured;
      this.#stopConfigure = () => {
        if (graph.configure === configured) graph.configure = configure;
      };
    } else this.#stopConfigure = () => {};
    this.reconcile();
  }

  /** Attach newly admitted nodes and retire removed handles after a real membership event. */
  reconcile(): void {
    const nodes = new Set(getGraphNodes(this.graph));
    for (const [node, stop] of this.#nodes)
      if (!nodes.has(node)) {
        stop();
        this.#nodes.delete(node);
      }
    for (const node of nodes) {
      if (this.#nodes.has(node)) continue;
      const previous = node.onConnectionsChange;
      const changed = (...args: unknown[]): unknown => {
        const result: unknown =
          typeof previous === 'function' ? previous.apply(node, args) : undefined;
        if (this.#active) this.changed(false);
        return result;
      };
      node.onConnectionsChange = changed;
      this.#nodes.set(node, () => {
        if (node.onConnectionsChange === changed) node.onConnectionsChange = previous;
      });
    }
  }

  /** Release all topology leases without disturbing callbacks installed by another native owner. */
  dispose(): void {
    this.#active = false;
    this.#membership?.dispose();
    this.#stopConfigure();
    for (const stop of this.#nodes.values()) stop();
    this.#nodes.clear();
  }
}
