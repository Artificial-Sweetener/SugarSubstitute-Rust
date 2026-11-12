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
/** Project resolved relation values into the existing outgoing Comfy queue candidate. */
import { isRecord } from '../../types/common.js';
import { NativeSubgraphBoundaryResolver } from '../../cube/graph/NativeSubgraphBoundaryResolver.js';
import type { WorkflowRelations } from '../application/WorkflowRelations.js';
import type { NativeRelationInventory } from './NativeRelationInventory.js';

/** Resolve editable facades to exact native prompt fields without enrolling hidden leaves. */
export class NativeRelationExecution {
  readonly #boundary: NativeSubgraphBoundaryResolver;

  /** Share the live candidate inventory with execution and renderer-independent policy. */
  constructor(
    private readonly relations: WorkflowRelations,
    private readonly inventory: NativeRelationInventory,
    logger: Pick<Console, 'warn'>,
    private readonly getWorkflow: () => unknown = () => undefined,
  ) {
    this.#boundary = new NativeSubgraphBoundaryResolver(logger);
  }

  /** Advance shared seeds once and rewrite only explicit eligible scalar prompt inputs. */
  prepare(payload: unknown): unknown {
    this.relations.advanceSeeds(
      () => (crypto.getRandomValues(new Uint32Array(1))[0] ?? 0) / 0x100000000,
    );
    const snapshot = this.relations.reconcile();
    if (!isRecord(payload) || !isRecord(payload.output)) return payload;
    const output = Object.fromEntries(
      Object.entries(payload.output).map(([id, node]) => [
        id,
        isRecord(node)
          ? { ...node, inputs: isRecord(node.inputs) ? { ...node.inputs } : {} }
          : node,
      ]),
    );
    for (const item of snapshot.resolved) {
      if (!item.override && !item.source) continue;
      this.inventory.prepareField(item.field.id, item.value);
      const owner = this.inventory.owners.get(item.field.id);
      if (!owner) continue;
      const slot =
        owner.face.inputs?.findIndex(
          (input) => input.widget?.name === item.field.name || input.name === item.field.name,
        ) ?? -1;
      const subgraph = owner.face.isSubgraphNode?.() === true;
      const targets =
        subgraph && slot >= 0
          ? this.#boundary.resolveInputTargets(owner.face, slot).map((target) => ({
              id:
                owner.cubeId === null
                  ? String(target.nodeId)
                  : `${owner.executionId.split(':')[0]}:${String(target.nodeId)}`,
              name: target.inputName,
            }))
          : [{ id: owner.executionId, name: item.field.name }];
      for (const target of targets) {
        const node: unknown = output[target.id];
        if (isRecord(node) && isRecord(node.inputs) && !Array.isArray(node.inputs[target.name]))
          node.inputs[target.name] = item.value;
      }
    }
    const workflow = this.getWorkflow();
    return { ...payload, output, ...(workflow === undefined ? {} : { workflow }) };
  }
}
