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
/** Define host-independent canonical graph facts and atomic reorder collaboration contracts. */

/** Identify one complete workflow snapshot, including geometry and live widget values. */
export interface CubeGraphSnapshot {
  readonly revision: string;
  readonly serializedWorkflow: string;
}

/** Carry stable workflow instance identity without retaining host node objects. */
export interface CubeGraphInstance {
  readonly instanceId: string;
  readonly nodeId: string;
  readonly definitionId: string;
  readonly cubeId: string;
  readonly version: string;
  readonly alias: string;
  readonly executionMode: number;
}

/** Describe one canonical segment and its explicit graph-boundary permissions. */
export interface CubeGraphSegment {
  readonly instanceIds: readonly string[];
  readonly reorderable: boolean;
  readonly boundaryNodeIds: readonly string[];
}

/** Carry a validated placement for an exact instance and native node pair. */
export interface CubeGraphPlacement {
  readonly instanceId: string;
  readonly nodeId: string;
  readonly position: readonly [number, number];
}

/** Expose only validated canonical facts consumed by workflow presentation. */
export interface CubeGraphAnalysis {
  readonly semanticHash: string;
  readonly instances: readonly CubeGraphInstance[];
  readonly segments: readonly CubeGraphSegment[];
  readonly placements: readonly CubeGraphPlacement[];
}

/** Request graph analysis and reorder from the authoritative topology owner. */
export interface CubeGraphGateway {
  analyze(snapshot: CubeGraphSnapshot): Promise<CubeGraphAnalysis>;
  reorder(
    snapshot: CubeGraphSnapshot,
    segmentInstanceIds: readonly string[],
    orderedInstanceIds: readonly string[],
  ): Promise<CubeGraphAnalysis>;
}

/** Keep snapshot capture and native history transactions at the host boundary. */
export interface CubeGraphReorderHost {
  readonly available: boolean;
  readonly revision: string;
  snapshot(): CubeGraphSnapshot;
  isCurrent(snapshot: CubeGraphSnapshot): boolean;
  /** Retain order authority through native presentation size changes. */
  isOrderCurrent(snapshot: CubeGraphSnapshot): boolean;
  isPlacementCurrent(snapshot: CubeGraphSnapshot): boolean;
  commit(snapshot: CubeGraphSnapshot, placements: readonly CubeGraphPlacement[]): boolean;
}
