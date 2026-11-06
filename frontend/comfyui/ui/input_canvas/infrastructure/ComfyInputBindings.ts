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
/** Retain one source-state owner per upload field per live node independently of masonry and renderer mount lifetimes. */
import type { ComfyNode } from '../../types/graph.js';
import type { InputLoader } from '../domain/InputLoader.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import type { InputBindingController } from '../application/InputBindingController.js';
import {
  createComfyInputBinding,
  isComfyInputSourceLinked,
  type ComfyInputBindingPorts,
} from './ComfyInputBinding.js';
import { getGraphNodes } from '../../graph/GraphQuery.js';
import { isRecord } from '../../types/common.js';

/** Restore bound-node observers at native workflow loading, with no DOM-driven graph discovery. */
export class ComfyInputBindings {
  readonly #nodes = new Map<
    ComfyNode,
    { bindings: Map<string, InputBindingController>; stop(): void }
  >();
  /** Share canvas and file adapters while keeping node callbacks scoped to the loaded workflow. */
  constructor(
    private readonly loaders: ComfyInputLoaders,
    private readonly ports: ComfyInputBindingPorts,
  ) {}
  /** Reuse one authoritative controller across every presentation of the same live node. */
  get(node: ComfyNode, loader: InputLoader): InputBindingController {
    let owner = this.#nodes.get(node);
    if (!owner) {
      const previous = node.onRemoved;
      const removed = (...args: unknown[]): unknown => {
        try {
          return typeof previous === 'function' ? previous.apply(node, args) : undefined;
        } finally {
          this.#release(node);
        }
      };
      node.onRemoved = removed;
      owner = {
        bindings: new Map(),
        stop: () => {
          if (node.onRemoved === removed) node.onRemoved = previous;
        },
      };
      this.#nodes.set(node, owner);
    }
    const current = owner.bindings.get(loader.field);
    if (current) return current;
    const binding = createComfyInputBinding(node, loader, this.ports);
    owner.bindings.set(loader.field, binding);
    binding.adoptFiles();
    void binding.initialize();
    return binding;
  }
  /** Restore selectable sources through Comfy's loaded-node lifecycle, including unconfigured loaders. */
  loaded(node: ComfyNode): void {
    this.#loaded(node, new Set());
  }
  /** Follow only the exact loaded instance's native descendants, rejecting recursive graph references. */
  #loaded(node: ComfyNode, seen: Set<ComfyNode>): void {
    if (seen.has(node)) return;
    seen.add(node);
    if (isRecord(node.subgraph))
      for (const child of getGraphNodes(node.subgraph)) this.#loaded(child, seen);
    for (const loader of this.loaders.all(node.type ?? node.class_type ?? '')) {
      if (isComfyInputSourceLinked(node, loader.field)) continue;
      try {
        this.get(node, loader);
      } catch (error: unknown) {
        this.ports.report(error);
      }
    }
  }

  /** Release node observers before a new workflow replaces their native instances. */
  clear(): void {
    for (const node of this.#nodes.keys()) this.#release(node);
  }
  /** Negotiate newly authenticated source capabilities for exact known loader instances. */
  async ready(): Promise<void> {
    await Promise.all(
      [...this.#nodes.values()].flatMap((owner) =>
        [...owner.bindings.values()].map((binding) => binding.initialize()),
      ),
    );
  }
  /** Restore only callbacks still owned by the retiring node instance. */
  #release(node: ComfyNode): void {
    const record = this.#nodes.get(node);
    if (!record) return;
    this.#nodes.delete(node);
    for (const binding of record.bindings.values()) binding.dispose();
    record.stop();
  }
}
