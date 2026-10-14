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
/** Identify a completed image independently of canvas implementation or batch navigation. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogSnapshot } from './CanvasCatalog.js';
import { canvasSessionId } from './CanvasIdentity.js';

export interface CanvasRevealTarget {
  readonly sessionId: WorkflowOutputItem['provenance']['sessionId'];
  readonly generation: number;
  readonly sourceId: WorkflowOutputItem['provenance']['sourceId'];
  readonly runOrder: number;
  readonly itemId: WorkflowOutputItem['id'];
  readonly batchIndex: number;
}

/** Capture immutable provenance when the thumbnail is presented, never when it is clicked. */
export function canvasRevealTarget(item: WorkflowOutputItem): CanvasRevealTarget {
  return Object.freeze({
    sessionId: item.provenance.sessionId,
    generation: item.provenance.generation,
    sourceId: item.provenance.sourceId,
    runOrder: item.provenance.runOrder,
    itemId: item.id,
    batchIndex: item.batchIndex,
  });
}

/** Refuse superseded runs and files rather than substituting the current image in the same slot. */
export function resolveCanvasReveal(
  catalog: CanvasCatalogSnapshot,
  target: CanvasRevealTarget,
): WorkflowOutputItem | null {
  if (
    catalog.sessionId !== canvasSessionId(target.sessionId) ||
    catalog.generation !== target.generation
  )
    return null;
  if (!catalog.sources.some((source) => source.id === target.sourceId)) return null;
  const item = catalog.result.outputs
    .find((entry) => entry.source.id === target.sourceId)
    ?.items.find(
      (candidate) => candidate.id === target.itemId && candidate.batchIndex === target.batchIndex,
    );
  return item?.mediaKind === 'image' &&
    item.provenance.sourceId === target.sourceId &&
    item.provenance.runOrder === target.runOrder &&
    item.provenance.sessionId === target.sessionId &&
    item.provenance.generation === target.generation
    ? item
    : null;
}
