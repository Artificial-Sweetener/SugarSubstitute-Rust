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
/**
 * Mark the phases of a workflow switch on the page's performance timeline.
 *
 * A switch passes through the host's graph configuration, the masonry rebuild and the output
 * canvas's attachment; each owner marks its own phase so a profile or a DevTools read of
 * `performance.getEntriesByType('mark')` shows where a slow switch spends its time.
 */
/** The prefix every switch mark carries. */
export const WORKFLOW_SWITCH_MARK = 'sugarcubes:switch:';
/** Record that a workflow switch reached `phase`. */
export function markWorkflowSwitch(phase) {
    globalThis.performance?.mark?.(`${WORKFLOW_SWITCH_MARK}${phase}`);
}
