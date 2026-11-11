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
/** Apply measured renderer geometry to one live cube from its authored baseline. */

import { readVector2 } from '../graph/VectorUtils.js';
import { isRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import {
  translateAuthoredLayoutBaseline,
  type AuthoredLayoutBaseline,
} from './AuthoredLayoutBaseline.js';
import {
  solveAuthoredGroupPresentationRect,
  toComfyGroupStorageRect,
} from './AuthoredGroupGeometry.js';
import { solveAuthoredLayout, type LayoutRect } from './AuthoredLayoutSolver.js';
import {
  authoredNodePresentationRect,
  measureNodePresentationRect,
  writeNodePresentationPosition,
} from './ComfyNodeGeometry.js';
import type { LiveGeometryInstance } from './LiveCubeGeometryIndex.js';
import type { NodeRenderer } from './RendererGeometryPolicy.js';

export interface InstanceGeometryMeasurementContext {
  renderer: NodeRenderer;
  document: Document | null;
}

/** Rebuild one live instance's node and group presentation from authored relations. */
export function applyMeasuredInstanceGeometry(
  instance: LiveGeometryInstance,
  measurement: InstanceGeometryMeasurementContext,
): boolean {
  const baseline = alignBaselineWithLiveGroup(instance);
  const solved = solveMeasuredNodeGeometry(baseline, instance.nodes, measurement);
  const nodesChanged = writeSolvedNodePositions(solved, measurement);
  const groupChanged = writeSolvedGroup(
    { ...instance, baseline },
    solved.map(({ solved: rect }) => rect),
    measurement.renderer,
  );
  return groupChanged || nodesChanged;
}

/** Rebuild native subgraph node positions from renderer-measured card geometry. */
export function applyMeasuredNodeGeometry(
  baseline: AuthoredLayoutBaseline,
  nodes: ReadonlyMap<string, ComfyNode>,
  measurement: InstanceGeometryMeasurementContext,
): boolean {
  return writeSolvedNodePositions(
    solveMeasuredNodeGeometry(baseline, nodes, measurement),
    measurement,
  );
}

/** Solve one node collection while preserving its renderer-neutral authored relations. */
function solveMeasuredNodeGeometry(
  baseline: AuthoredLayoutBaseline,
  nodes: ReadonlyMap<string, ComfyNode>,
  measurement: InstanceGeometryMeasurementContext,
) {
  const measurements = Object.entries(baseline.entries)
    .map(([identity, entry]) => {
      const node = nodes.get(identity);
      const measured = node ? measureNodePresentationRect(node, measurement) : null;
      if (!node || !measured) return null;
      const authored = authoredNodePresentationRect(
        {
          x: baseline.origin[0] + entry.x,
          y: baseline.origin[1] + entry.y,
          w: entry.w,
          h: entry.h,
        },
        entry.collapsed,
        entry.title,
      );
      return { identity, item: node, authored, measured: { w: measured.w, h: measured.h } };
    })
    .filter((value): value is NonNullable<typeof value> => value !== null);
  return solveAuthoredLayout(measurements);
}

/** Write solved presentation positions through Comfy's renderer convention. */
function writeSolvedNodePositions(
  solved: ReturnType<typeof solveMeasuredNodeGeometry>,
  measurement: InstanceGeometryMeasurementContext,
): boolean {
  let changed = false;
  for (const item of solved) {
    const current = measureNodePresentationRect(item.item, measurement);
    if (!current || !rectPositionEqual(current, item.solved)) {
      writeNodePresentationPosition(item.item, item.solved, measurement.renderer);
      changed = true;
    }
  }
  return changed;
}

/** Preserve instance translation while keeping authored shape data unchanged. */
function alignBaselineWithLiveGroup(instance: LiveGeometryInstance): AuthoredLayoutBaseline {
  const authoredGroup = instance.baseline.group;
  if (!authoredGroup) return instance.baseline;
  const livePos = readVector2(instance.group.pos, Number.NaN, Number.NaN);
  if (!livePos.every(Number.isFinite)) return instance.baseline;
  const expectedX = instance.baseline.origin[0] + authoredGroup.x;
  const expectedY = instance.baseline.origin[1] + authoredGroup.y;
  return translateAuthoredLayoutBaseline(instance.baseline, [
    livePos[0] - expectedX,
    livePos[1] - expectedY,
  ]);
}

function writeSolvedGroup(
  instance: LiveGeometryInstance,
  solvedRects: LayoutRect[],
  renderer: NodeRenderer,
): boolean {
  const presentation = solveAuthoredGroupPresentationRect(instance.baseline, solvedRects);
  if (!presentation) return false;
  const target = toComfyGroupStorageRect(presentation, renderer);
  const currentPos = readVector2(instance.group.pos, Number.NaN, Number.NaN);
  const currentSize = readVector2(instance.group.size, Number.NaN, Number.NaN);
  const changed =
    !nearlyEqual(currentPos[0], target.x) ||
    !nearlyEqual(currentPos[1], target.y) ||
    !nearlyEqual(currentSize[0], target.w) ||
    !nearlyEqual(currentSize[1], target.h);
  instance.group.pos = [target.x, target.y];
  instance.group.size = [target.w, target.h];
  const currentBounds = isRecord(instance.metadata.bounds) ? instance.metadata.bounds : {};
  instance.metadata.bounds = { ...currentBounds, ...target };
  return changed;
}

function rectPositionEqual(left: LayoutRect, right: LayoutRect): boolean {
  return nearlyEqual(left.x, right.x) && nearlyEqual(left.y, right.y);
}

function nearlyEqual(left: number, right: number): boolean {
  return Math.abs(left - right) < 0.001;
}
