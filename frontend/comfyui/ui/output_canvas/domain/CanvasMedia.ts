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
/** Describe renderer-neutral media actions and pin context commands to exact output identities. */
import type { OutputSourceId, OutputResultId } from '../../execution/domain/ExecutionIdentity.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogEntry } from './CanvasCatalog.js';
import type { CanvasViewSnapshot } from './CanvasViewPort.js';

export type CanvasMediaTarget =
  | 'primary'
  | 'secondary'
  | {
      readonly sourceId: OutputSourceId;
      readonly itemId: OutputResultId;
    };

/** Per-final actions derived from the actual byte backing and existing transfer owner's limits. */
export interface CanvasOutputActions {
  readonly itemId: OutputResultId;
  readonly copy: boolean;
  readonly file: boolean;
  readonly drag: boolean;
  readonly photoshop: boolean;
  readonly fileAction: 'reveal' | 'download';
}

/** Resolve only the exact current catalog item rather than reusing a batch index across generations. */
export function canvasMediaItem(
  snapshot: CanvasViewSnapshot,
  target: CanvasMediaTarget,
): WorkflowOutputItem | null {
  const item = canvasMediaFile(snapshot, target);
  return item?.mediaKind === 'image' ? item : null;
}

/** Resolve an exact ready final of any supported kind for its original-file action. */
export function canvasMediaFile(
  snapshot: CanvasViewSnapshot,
  target: CanvasMediaTarget,
): WorkflowOutputItem | null {
  const item =
    target === 'primary'
      ? !snapshot.preview &&
        snapshot.view.mode !== 'overview' &&
        snapshot.resource.state === 'ready'
        ? snapshot.selected
        : null
      : target === 'secondary'
        ? snapshot.secondaryResource?.state === 'ready'
          ? snapshot.secondary
          : null
        : (snapshot.grid
            .find((entry) => entry.source.id === target.sourceId)
            ?.items.find((item) => item.id === target.itemId) ?? null);
  return item && item.mediaKind !== 'unsupported' && !item.unavailable ? item : null;
}

/** List every still image of a result's sources in catalog order, excluding live previews. */
export function canvasMediaImages(
  entries: readonly CanvasCatalogEntry[],
): readonly WorkflowOutputItem[] {
  return entries.flatMap((entry) => entry.items.filter((item) => item.mediaKind === 'image'));
}
