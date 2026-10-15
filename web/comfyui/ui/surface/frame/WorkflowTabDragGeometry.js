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
/** Resolve drag previews from committed slots, independent of animated browser geometry. */
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
/** Move to the held tab's nearest slot with a small dead band to prevent boundary chatter. */
export function workflowTabDragPlacement(count, origin, delta, previousTarget = origin) {
    const width = WORKFLOW_TAB_WIDTH;
    const draggedLeft = Math.max(0, Math.min((count - 1) * width, origin * width + delta));
    let target = Math.max(0, Math.min(count - 1, previousTarget));
    while (target < count - 1 && draggedLeft >= (target + 0.5) * width + 6)
        target++;
    while (target > 0 && draggedLeft <= (target - 0.5) * width - 6)
        target--;
    const order = Array.from({ length: count }, (_, index) => index);
    order.splice(origin, 1);
    order.splice(target, 0, origin);
    return {
        target,
        draggedLeft,
        offsets: Array.from({ length: count }, (_, index) => index === origin ? draggedLeft - origin * width : (order.indexOf(index) - index) * width),
    };
}
