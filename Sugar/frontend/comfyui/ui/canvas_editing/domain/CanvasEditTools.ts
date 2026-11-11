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
/** Arrange editing tools into the rail's ordered slots and resolve each slot's presented member. */
import type {
  CanvasEditState,
  CanvasToolId,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

export type CanvasEditSlotId =
  | 'navigation'
  | 'move'
  | 'brush'
  | 'eraser'
  | 'selection'
  | 'mask-shapes'
  | 'transform';

/** One rail position; a slot with several members presents them as a flyout group. */
export interface CanvasEditSlot {
  readonly id: CanvasEditSlotId;
  readonly members: readonly CanvasToolId[];
}

/** The rail order of the reference input canvas, without its segmentation tools. */
export const CANVAS_EDIT_SLOTS: readonly CanvasEditSlot[] = Object.freeze([
  { id: 'navigation', members: ['pan-zoom'] },
  { id: 'move', members: ['move'] },
  { id: 'brush', members: ['brush'] },
  { id: 'eraser', members: ['eraser'] },
  { id: 'selection', members: ['select-rectangle', 'select-ellipse', 'select-lasso'] },
  { id: 'mask-shapes', members: ['mask-rectangle', 'mask-ellipse', 'mask-lasso'] },
  { id: 'transform', members: ['transform', 'shared-edges'] },
]);

export interface CanvasEditMemberView {
  readonly id: CanvasToolId;
  readonly available: boolean;
  readonly active: boolean;
}

/** Present one slot: the member its button shows and activates, and every offered member. */
export interface CanvasEditSlotView {
  readonly id: CanvasEditSlotId;
  readonly current: CanvasEditMemberView;
  readonly members: readonly CanvasEditMemberView[];
  readonly grouped: boolean;
}

/**
 * Resolve the rail against the canvas's offered tools.
 *
 * Tools the canvas does not list are omitted; listed but unavailable tools stay visible and
 * disabled. A slot shows its active member, else the member last chosen from its flyout, else
 * its first offered member.
 */
export function canvasEditSlotViews(
  state: Pick<CanvasEditState, 'tool' | 'tools'>,
  remembered: ReadonlyMap<CanvasEditSlotId, CanvasToolId>,
): readonly CanvasEditSlotView[] {
  const offered = new Map(state.tools.map((tool) => [tool.id, tool.available]));
  const views: CanvasEditSlotView[] = [];
  for (const slot of CANVAS_EDIT_SLOTS) {
    const members = slot.members.flatMap((id) => {
      const available = offered.get(id);
      return available === undefined ? [] : [{ id, available, active: state.tool === id }];
    });
    const current =
      members.find((member) => member.active) ??
      members.find((member) => member.id === remembered.get(slot.id)) ??
      members[0];
    if (current) views.push({ id: slot.id, current, members, grouped: members.length > 1 });
  }
  return views;
}

/** Find the slot that owns a tool, so a flyout choice can be remembered for its slot. */
export function canvasEditSlotOf(tool: CanvasToolId): CanvasEditSlotId | null {
  return CANVAS_EDIT_SLOTS.find((slot) => slot.members.includes(tool))?.id ?? null;
}

/** Report whether the canvas offers a tool and currently allows it. */
export function canvasEditToolAvailable(
  state: Pick<CanvasEditState, 'tools'>,
  tool: CanvasToolId,
): boolean {
  return state.tools.some((entry) => entry.id === tool && entry.available);
}
