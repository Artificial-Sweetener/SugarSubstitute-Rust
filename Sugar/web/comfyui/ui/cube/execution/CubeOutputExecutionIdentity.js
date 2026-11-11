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
/** Own stable prompt-only identities for loaded Cube output boundary semantics. */
const CUBE_OUTPUT_EXECUTION_PREFIX = '__sugarcubes_cube_output__';
const CUBE_OUTPUT_EXECUTION_LEADER = `${CUBE_OUTPUT_EXECUTION_PREFIX}:`;
/** Build the prompt and media-map key for one ordered Cube output boundary. */
export function buildCubeOutputExecutionId(cubeNodeId, outputSlot) {
    if (!Number.isInteger(outputSlot) || outputSlot < 0) {
        throw new TypeError('Cube output slot must be a non-negative integer.');
    }
    return `${CUBE_OUTPUT_EXECUTION_PREFIX}:${String(cubeNodeId)}:${String(outputSlot)}`;
}
/** Identify execution events produced by SugarCubes' prompt-only output sinks. */
export function isCubeOutputExecutionId(value) {
    if (typeof value !== 'string' || !value.startsWith(CUBE_OUTPUT_EXECUTION_LEADER))
        return false;
    const slotSeparator = value.lastIndexOf(':');
    if (slotSeparator <= CUBE_OUTPUT_EXECUTION_LEADER.length)
        return false;
    const slot = value.slice(slotSeparator + 1);
    return /^(?:0|[1-9]\d*)$/.test(slot);
}
