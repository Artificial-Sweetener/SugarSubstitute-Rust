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
/** Take the seeds one job ran with into the workflow on screen. */
import { recordExactIntegers } from '../../workflow/ExactIntegerRelations.js';
import { semanticFieldName } from '../domain/FieldCompatibility.js';
import type { WorkflowRelations } from '../application/WorkflowRelations.js';
import type { NativeRelationInventory } from './NativeRelationInventory.js';

/** One seed a job gave one node of one named Cube, as exact decimal digits. */
export interface JobSeed {
  readonly alias: string;
  readonly nodeSymbol: string;
  readonly inputName: string;
  readonly digits: string;
}

/**
 * Give the shown workflow's seeds the values a job used.
 *
 * A job's Cubes are matched to the workflow's by name and node, so the seeds of a run made in
 * one tab can be taken into another that holds the same Cubes. A seed every Cube shares is set
 * once on the shared seed; each is held fixed, as a seed taken to be reproduced should be. The
 * digits a page number cannot hold are recorded for the run, as for any exact seed.
 */
export class NativeJobSeedAdoption {
  /** Read the root workflow and the relation inventory instead of browser state. */
  constructor(
    private readonly getGraph: () => unknown,
    private readonly relations: WorkflowRelations,
    private readonly inventory: NativeRelationInventory,
  ) {}

  /** Apply the job's seeds; returns how many of the workflow's seed fields took one. */
  adopt(seeds: readonly JobSeed[]): number {
    const snapshot = this.relations.reconcile();
    const shared = new Set<string>();
    const exact: { instanceId: string; nodeSymbol: string; inputName: string; digits: string }[] =
      [];
    let adopted = 0;
    for (const field of snapshot.fields) {
      if (field.semantic !== 'seed') continue;
      const owner = this.inventory.owners.get(field.id);
      const symbol = owner ? nodeSymbol(owner.nodeId) : null;
      const alias = snapshot.nodes.find((node) => node.id === field.nodeId)?.cubeLabel;
      const seed = seeds.find(
        (candidate) =>
          candidate.alias === alias &&
          candidate.nodeSymbol === symbol &&
          semanticFieldName(candidate.inputName) === 'seed' &&
          /^(?:0|[1-9][0-9]{0,39})$/u.test(candidate.digits),
      );
      if (!owner || symbol === null || !seed) continue;
      adopted += 1;
      if (owner.cubeId)
        exact.push({
          instanceId: owner.cubeId,
          nodeSymbol: symbol,
          inputName: owner.inputName,
          digits: seed.digits,
        });
      // A seed every Cube shares is one value: the first matching Cube's seed sets it.
      const override = snapshot.resolved.find((item) => item.field.id === field.id)?.override;
      if (override) {
        if (shared.has(override)) continue;
        shared.add(override);
      }
      this.relations.reuseSeed(field.id, Number(seed.digits));
    }
    const graph = this.getGraph();
    if (exact.length && typeof graph === 'object' && graph !== null)
      recordExactIntegers(graph, exact);
    return adopted;
  }
}

/** Read the Cube node symbol from a relation node identity. */
function nodeSymbol(nodeId: string): string | null {
  try {
    const parts: unknown = JSON.parse(nodeId);
    return Array.isArray(parts) && parts[0] === 'cube' && typeof parts[2] === 'string'
      ? parts[2]
      : null;
  } catch {
    return null;
  }
}
