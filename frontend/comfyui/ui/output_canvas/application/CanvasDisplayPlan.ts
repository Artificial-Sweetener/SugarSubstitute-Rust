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
/** Describe visible logical image interests without retaining platform handles or preview bytes. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { OutputSourceId } from '../../execution/domain/ExecutionIdentity.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection, CanvasViewState } from '../domain/CanvasViewState.js';
import type { CanvasPinnedPair } from './CanvasResultPins.js';

export interface CanvasDisplaySlot {
  readonly selection: CanvasSelection;
  readonly item: WorkflowOutputItem | null;
  readonly sourcePresent: boolean;
}

export interface CanvasDisplayPlan {
  readonly view: CanvasViewState;
  readonly generation: number;
  readonly signature: string;
  readonly availableSources: readonly OutputSourceId[];
  readonly primary: CanvasDisplaySlot | null;
  readonly secondary: CanvasDisplaySlot | null;
  readonly preview: WorkflowPreview | null;
}

/** Qualify lightweight preview admission to one source/run rather than each transport frame. */
export function canvasPreviewLane(preview: WorkflowPreview): string {
  return JSON.stringify([
    'preview',
    preview.sessionId,
    preview.generation,
    preview.promptId,
    preview.sourceId,
    preview.runOrder,
  ]);
}

/** Keep camera, wipe, labels, and preview sequence changes out of complete resource-plan replacement. */
export function canvasDisplayPlan(
  view: CanvasViewState,
  catalog: CanvasCatalogSnapshot,
  items: CanvasPinnedPair,
  preview: WorkflowPreview | null,
): CanvasDisplayPlan {
  if (view.sessionId !== catalog.sessionId)
    throw new TypeError('Canvas display catalog belongs to another view.');
  const slot = (
    selection: CanvasSelection | null,
    item: WorkflowOutputItem | null,
  ): CanvasDisplaySlot | null =>
    selection
      ? Object.freeze({
          selection,
          item,
          sourcePresent: catalog.entries.some((entry) => entry.source.id === selection.sourceId),
        })
      : null;
  const primary = slot(view.selection, items.primary);
  const secondary = slot(view.comparison?.secondary ?? null, items.secondary);
  const availableSources = Object.freeze(catalog.entries.map((entry) => entry.source.id));
  const signature = JSON.stringify([
    catalog.generation,
    view.mode,
    availableSources,
    primary?.selection,
    primary?.sourcePresent,
    primary?.item?.id,
    secondary?.selection,
    secondary?.sourcePresent,
    secondary?.item?.id,
    preview ? canvasPreviewLane(preview) : null,
  ]);
  return Object.freeze({
    view,
    generation: catalog.generation,
    signature,
    availableSources,
    primary,
    secondary,
    preview,
  });
}
