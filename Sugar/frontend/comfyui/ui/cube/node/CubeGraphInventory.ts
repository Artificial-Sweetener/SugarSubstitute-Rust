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
/** Classify root SugarCubes separately from invalid wrappers in native subgraph definitions. */

import { isRecord } from '../../types/common.js';
import { isCubeNode, type CubeNode } from './ComfyCubeNodeFactory.js';

export interface CubeInventoryGraph {
  _nodes?: unknown[];
  subgraphs?: ReadonlyMap<string, unknown>;
}

export interface NestedCubeViolation {
  definitionId: string;
  definitionName: string;
  node: CubeNode;
}

export interface CubeGraphInventorySnapshot {
  rootCubes: readonly CubeNode[];
  nestedCubes: readonly NestedCubeViolation[];
}

/** Identify invalid persisted nesting at execution and persistence boundaries. */
export class NestedCubeWorkflowError extends Error {
  readonly code = 'SUGARCUBE_NESTED_WORKFLOW';
  readonly violations: readonly NestedCubeViolation[];

  /** Preserve violation evidence while giving the user a direct recovery action. */
  constructor(action: string, violations: readonly NestedCubeViolation[]) {
    const count = violations.length;
    super(
      `Cannot ${action}: ${String(count)} SugarCube${count === 1 ? '' : 's'} ` +
        `${count === 1 ? 'is' : 'are'} nested inside a Subgraph. ` +
        'Remove the nested SugarCube wrapper and keep ordinary Subgraphs for reusable inner graphs.',
    );
    this.name = 'NestedCubeWorkflowError';
    this.violations = violations;
  }
}

/** Read the authoritative root and definition registries without changing workflow data. */
export class CubeGraphInventory {
  readonly #rootGraph: CubeInventoryGraph;

  /** Bind one workflow root for its complete runtime lifecycle. */
  constructor(rootGraph: CubeInventoryGraph) {
    this.#rootGraph = rootGraph;
  }

  /** Return current root Cubes and recoverable invalid nested wrappers. */
  snapshot(): CubeGraphInventorySnapshot {
    const rootCubes = (this.#rootGraph._nodes ?? []).filter(isCubeNode);
    const nestedCubes: NestedCubeViolation[] = [];
    for (const [registeredId, value] of this.#rootGraph.subgraphs?.entries() ?? []) {
      if (!isRecord(value)) continue;
      const definitionId = readString(value.id) || registeredId;
      const definitionName = readString(value.name) || definitionId;
      const nodes = Array.isArray(value._nodes) ? value._nodes : [];
      for (const node of nodes) {
        if (isCubeNode(node)) nestedCubes.push({ definitionId, definitionName, node });
      }
    }
    return { rootCubes, nestedCubes };
  }

  /** Reject execution or persistence while leaving invalid historical data untouched. */
  assertNoNestedCubes(action: string): void {
    const { nestedCubes } = this.snapshot();
    if (nestedCubes.length) throw new NestedCubeWorkflowError(action, nestedCubes);
  }
}

/** Read one non-empty host string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
