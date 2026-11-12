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
/** Materialize host-neutral workflow plans as native connected Cubes. */

import type { CubePlacementService, PlacedCube } from '../cube/CubePlacementService.js';
import { readCubeBoundaryDefinitions } from '../cube/CubeBoundaryDefinitions.js';
import type { ImportPayload } from '../import/PlacementPayload.js';
import type { Vec2 } from '../types/common.js';
import type {
  SugarScriptNativeConnection,
  SugarScriptNativeWorkflowPlan,
} from '../sugarscript/SugarScriptAuthoringModels.js';
import { SugarScriptWorkflowCompanion } from '../sugarscript/SugarScriptWorkflowCompanion.js';
import type { WorkflowExtraOwner } from '../sugarscript/SugarScriptWorkflowCompanion.js';

const COLUMN_SPACING = 1000;
const ROW_SPACING = 700;
const COLUMNS = 3;

export interface NativeWorkflowMaterializationResult {
  semanticHash: string;
  instanceIds: readonly string[];
  warnings: readonly string[];
}

interface MaterializedCube {
  placed: PlacedCube;
  payload: ImportPayload;
}

export interface NativeNestedDefinitionRegistry {
  register(payload: ImportPayload): { warnings: string[]; createdIds: string[] };
  discard(ids: readonly string[]): void;
}

/** Own atomic placement, bypass projection, Cube connections, and companions. */
export class NativeWorkflowMaterializer {
  readonly #placement: CubePlacementService;
  readonly #companion: SugarScriptWorkflowCompanion;
  readonly #subgraphs: NativeNestedDefinitionRegistry;

  /** Bind normal Cube placement and authoritative workflow metadata owners. */
  constructor(
    placement: CubePlacementService,
    graph: WorkflowExtraOwner,
    subgraphs: NativeNestedDefinitionRegistry,
  ) {
    this.#placement = placement;
    this.#companion = new SugarScriptWorkflowCompanion(graph);
    this.#subgraphs = subgraphs;
  }

  /** Create one native SugarScript workflow and attach its source companion. */
  materializeSugarScript(
    plan: SugarScriptNativeWorkflowPlan,
    source: string,
    origin: Vec2,
  ): NativeWorkflowMaterializationResult {
    return this.#materialize(plan, origin, () => this.#companion.attach(source, plan.semanticHash));
  }

  /** Create one native workflow from an already reconciled persisted artifact. */
  materializeWorkflow(
    plan: SugarScriptNativeWorkflowPlan,
    origin: Vec2,
  ): NativeWorkflowMaterializationResult {
    return this.#materialize(plan, origin, () => undefined);
  }

  /** Create one native batch or restore graph and companion state on failure. */
  #materialize(
    plan: SugarScriptNativeWorkflowPlan,
    origin: Vec2,
    attachCompanion: () => void,
  ): NativeWorkflowMaterializationResult {
    const snapshot = this.#companion.capture();
    const createdSubgraphIds = new Set<string>();
    try {
      const registrationWarnings: string[] = [];
      for (const instance of plan.instances) {
        const registration = this.#subgraphs.register(instance.payload);
        registrationWarnings.push(...registration.warnings);
        for (const id of registration.createdIds) createdSubgraphIds.add(id);
      }
      const placed = this.#placement.placeBatch(
        plan.instances.map((instance, index) => ({
          payload: instance.payload,
          options: {
            instanceAlias: instance.alias,
            instanceId: instance.instanceId,
            position: positionFor(origin, index),
          },
        })),
        (items) => {
          const byId = new Map<string, MaterializedCube>();
          for (const [index, instance] of plan.instances.entries()) {
            const placed = items[index];
            if (!placed) throw new Error(`Workflow Cube '${instance.alias}' was not placed.`);
            byId.set(instance.instanceId, { placed, payload: instance.payload });
            if (instance.bypassed) placed.node.mode = 4;
          }
          for (const connection of plan.connections) connectCubes(connection, byId);
          attachCompanion();
        },
      );
      return {
        semanticHash: plan.semanticHash,
        instanceIds: plan.instances.map((instance) => instance.instanceId),
        warnings: [...registrationWarnings, ...placed.flatMap((item) => item.warnings)],
      };
    } catch (error: unknown) {
      this.#subgraphs.discard([...createdSubgraphIds]);
      this.#companion.restore(snapshot);
      throw error;
    }
  }
}

/** Connect one exact public Cube boundary pair. */
function connectCubes(
  connection: SugarScriptNativeConnection,
  placed: ReadonlyMap<string, MaterializedCube>,
): void {
  const source = placed.get(connection.sourceInstanceId);
  const target = placed.get(connection.targetInstanceId);
  if (!source || !target) throw new Error('Workflow connection references a missing Cube.');
  const outputSlot = canonicalBoundarySlot(source, 'output', connection.sourceBinding);
  const inputSlot = canonicalBoundarySlot(target, 'input', connection.targetBinding);
  source.placed.node.connect(outputSlot, target.placed.node, inputSlot);
}

/** Resolve canonical Sugar identities to the native slot order created from that contract. */
function canonicalBoundarySlot(
  cube: MaterializedCube,
  direction: 'input' | 'output',
  name: string,
): number {
  const title = String(cube.placed.node.title ?? cube.placed.node.id);
  const nativeBoundaries =
    direction === 'input' ? cube.placed.subgraph.inputs : cube.placed.subgraph.outputs;
  const contract = readCubeBoundaryDefinitions(cube.payload.boundaries);
  if (!contract) {
    if (cube.payload.boundaries !== undefined) {
      throw new Error(`Cube '${title}' has an invalid canonical boundary contract.`);
    }
    return uniqueBoundarySlot(nativeBoundaries, name, title);
  }
  const definitions = direction === 'input' ? contract.inputs : contract.outputs;
  const matches = definitions.flatMap((boundary, index) => (boundary.name === name ? [index] : []));
  if (matches.length !== 1) {
    throw new Error(`Cube '${title}' does not expose one boundary named '${name}'.`);
  }
  const slot = matches[0];
  if (slot === undefined || nativeBoundaries[slot] === undefined) {
    throw new Error(`Cube '${title}' boundary '${name}' is unavailable.`);
  }
  return slot;
}

/** Resolve a unique canonical boundary name without suffix guessing. */
function uniqueBoundarySlot(
  boundaries: readonly { name: string }[],
  name: string,
  cubeTitle: string,
): number {
  const matches = boundaries.flatMap((boundary, index) => (boundary.name === name ? [index] : []));
  if (matches.length !== 1) {
    throw new Error(`Cube '${cubeTitle}' does not expose one boundary named '${name}'.`);
  }
  const slot = matches[0];
  if (slot === undefined) throw new Error(`Cube '${cubeTitle}' boundary '${name}' is unavailable.`);
  return slot;
}

/** Lay out source-ordered Cubes predictably without encoding execution order. */
function positionFor(origin: Vec2, index: number): Vec2 {
  return [
    origin[0] + (index % COLUMNS) * COLUMN_SPACING,
    origin[1] + Math.floor(index / COLUMNS) * ROW_SPACING,
  ];
}
