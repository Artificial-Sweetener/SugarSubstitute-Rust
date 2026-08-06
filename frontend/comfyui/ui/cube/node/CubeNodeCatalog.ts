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
/** Index live native Cube nodes without owning their graph lifecycle. */

import { isCubeNode, requireCubeIdentity, type CubeNode } from './ComfyCubeNodeFactory.js';

/** Provide stable instance lookup while Comfy's graph registry remains authoritative. */
export class CubeNodeCatalog {
  readonly #nodes = new Map<string, CubeNode>();
  readonly #listeners = new Set<() => void>();

  /** Index one graph-owned Cube node. */
  add(node: CubeNode): void {
    const instanceId = readInstanceId(node);
    const existing = this.#nodes.get(instanceId);
    if (existing === node) return;
    if (existing) {
      throw new Error(`Cube node '${instanceId}' already exists.`);
    }
    this.#nodes.set(instanceId, node);
    this.#notify();
  }

  /** Rebuild the index from Comfy's authoritative graph-node collection. */
  replace(nodes: Iterable<unknown>): void {
    const replacement = new Map<string, CubeNode>();
    for (const value of nodes) {
      if (!isCubeNode(value)) continue;
      const instanceId = readInstanceId(value);
      if (replacement.has(instanceId)) {
        throw new Error(`Workflow contains duplicate Cube node '${instanceId}'.`);
      }
      replacement.set(instanceId, value);
    }
    if (sameEntries(this.#nodes, replacement)) return;
    this.#nodes.clear();
    for (const [instanceId, node] of replacement) this.#nodes.set(instanceId, node);
    this.#notify();
  }

  /** Stop indexing one instance after Comfy removes its native node. */
  remove(instanceId: string): CubeNode | null {
    const node = this.#nodes.get(instanceId) ?? null;
    if (node) {
      this.#nodes.delete(instanceId);
      this.#notify();
    }
    return node;
  }

  /** Return one graph-owned Cube node by stable instance identity. */
  get(instanceId: string): CubeNode | null {
    return this.#nodes.get(instanceId) ?? null;
  }

  /** Notify presentation after a native node's Cube-owned face state changes. */
  changed(node: CubeNode): void {
    if (this.#nodes.get(readInstanceId(node)) === node) this.#notify();
  }

  /** Return graph-owned Cube nodes in stable discovery order. */
  list(): readonly CubeNode[] {
    return [...this.#nodes.values()];
  }

  /** Observe index and face-state changes. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Notify subscribers after one coherent catalog change. */
  #notify(): void {
    for (const listener of this.#listeners) listener();
  }
}

/** Compare stable identity and order without waking presentation for an unchanged graph. */
function sameEntries(
  current: ReadonlyMap<string, CubeNode>,
  replacement: ReadonlyMap<string, CubeNode>,
): boolean {
  if (current.size !== replacement.size) return false;
  const currentEntries = [...current.entries()];
  const replacementEntries = [...replacement.entries()];
  return currentEntries.every(([instanceId, node], index) => {
    const replacementEntry = replacementEntries[index];
    return replacementEntry?.[0] === instanceId && replacementEntry[1] === node;
  });
}

/** Require the stable extension instance identity stored on one native node. */
export function readInstanceId(node: CubeNode): string {
  const value = requireCubeIdentity(node).instance_id;
  const instanceId = typeof value === 'string' ? value.trim() : '';
  if (!instanceId) {
    throw new TypeError(`Cube node '${String(node.id)}' has no stable instance identity.`);
  }
  return instanceId;
}
