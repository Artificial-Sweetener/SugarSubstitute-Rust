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
import type { CanvasViewSnapshot } from './CanvasViewPort.js';

export type CanvasMediaTarget =
  | 'primary'
  | 'secondary'
  | {
      readonly sourceId: OutputSourceId;
      readonly itemId: OutputResultId;
    };

/** Resolve only the exact current catalog item rather than reusing a batch index across generations. */
export function canvasMediaItem(
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
  return item?.mediaKind === 'image' ? item : null;
}
