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
/** Give each Cube exclusive native definitions so editable values have one instance owner. */
import type { NativeCubeSubgraph } from '../ComfyCubeGraphBuilder.js';
import { isNativeCubeSubgraph } from '../ComfyCubeRuntimeGraphBoundary.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import type { ComfyNode } from '../../types/graph.js';
import { isRecord } from '../../types/common.js';
import { cloneNativeCubeDefinition } from './NativeCubeDefinitionClone.js';
import { transferCubeInstanceState } from '../node/CubeInstanceStateTransfer.js';
import { isCubeNode } from '../node/ComfyCubeNodeFactory.js';

interface DefinitionAssignment {
  readonly node: ComfyNode;
  readonly original: NativeCubeSubgraph;
  readonly originalType: string | undefined;
  readonly originalSurface: unknown;
}

/** Isolate host-shared root and nested definitions before Cube consumers discover editable owners. */
export class NativeCubeDefinitionOwnership {
  /** Receive native registration and identity generation from the Cube lifecycle boundary. */
  constructor(
    private readonly graph: object,
    private readonly createId: () => string,
    private readonly discardDefinition: (id: string) => void,
  ) {}

  /** Retain Cube root identities, slots and connections while separating copied mutable contents. */
  reconcile(cubes: readonly CubeNode[]): void {
    const owners = new Map<NativeCubeSubgraph, CubeNode>();
    const assignments: DefinitionAssignment[] = [];
    const created: string[] = [];
    const visit = (node: ComfyNode, owner: CubeNode): void => {
      const original = node.subgraph;
      if (!isNativeCubeSubgraph(original)) return;
      const previous = owners.get(original);
      if (previous === owner) return;
      let definition = original;
      if (previous) {
        definition = this.#clone(original, created);
        const originalSurface = node.properties?.sugarcubes_surface;
        assignments.push({ node, original, originalType: node.type, originalSurface });
        const source = isCubeNode(node)
          ? { ...node, properties: { ...node.properties }, subgraph: original }
          : null;
        this.#assign(node, definition);
        if (source && isCubeNode(node)) {
          node.properties.sugarcubes_surface = {};
          transferCubeInstanceState(source, node);
        }
      }
      owners.set(definition, owner);
      for (const child of definition._nodes) visit(child, owner);
    };
    try {
      for (const cube of cubes) visit(cube, cube);
    } catch (error: unknown) {
      const failures: unknown[] = [error];
      for (const assignment of assignments.reverse()) {
        try {
          this.#assign(assignment.node, assignment.original);
          if (assignment.originalType === undefined)
            Reflect.deleteProperty(assignment.node, 'type');
          else assignment.node.type = assignment.originalType;
          if (assignment.node.properties) {
            if (assignment.originalSurface === undefined)
              Reflect.deleteProperty(assignment.node.properties, 'sugarcubes_surface');
            else assignment.node.properties.sugarcubes_surface = assignment.originalSurface;
          }
        } catch (restoreError: unknown) {
          failures.push(restoreError);
        }
      }
      for (const id of created) {
        try {
          this.discardDefinition(id);
        } catch (discardError: unknown) {
          failures.push(discardError);
        }
      }
      if (failures.length > 1)
        throw new AggregateError(
          failures,
          'Cube definition isolation could not be fully restored.',
        );
      throw error;
    }
  }

  /** Register a fully serialized native copy through Comfy's public definition-creation API. */
  #clone(original: NativeCubeSubgraph, created: string[]): NativeCubeSubgraph {
    const create: unknown = Reflect.get(this.graph, 'createSubgraph');
    const registry: unknown = Reflect.get(this.graph, 'subgraphs');
    if (typeof create !== 'function' || !(registry instanceof Map) || !original.asSerialisable)
      throw new TypeError('Comfy native definition isolation is unavailable for copied Cubes.');
    const data = cloneNativeCubeDefinition(original, this.createId);
    const id = this.createId().trim();
    if (!id || registry.has(id)) throw new Error('Copied Cube definition identity is unavailable.');
    data.id = id;
    created.push(id);
    const definition: unknown = create.call(this.graph, data);
    if (!isNativeCubeSubgraph(definition) || definition === original || definition.id !== id)
      throw new TypeError('Comfy did not create an independent native Cube definition.');
    definition.configure(data);
    return definition;
  }

  /** Rebind native promoted slots after changing the definition while preserving external links. */
  #assign(node: ComfyNode, definition: NativeCubeSubgraph): void {
    node.subgraph = definition;
    node.type = definition.id;
    // Comfy publishes promoted binding refresh on SubgraphNode, but keeps slot rebinding internal.
    const rebind: unknown = Reflect.get(node, '_internalConfigureAfterSlots');
    if (typeof rebind === 'function') rebind.call(node);
    else if (isRecord(node) && typeof node.rebuildInputWidgetBindings === 'function')
      node.rebuildInputWidgetBindings.call(node);
  }
}
