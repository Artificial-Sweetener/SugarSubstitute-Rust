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
/** Map runtime Cube slots to stable boundary identities across versions. */
import { buildCubePayloadTopology } from '../CubePayloadTopology.js';
import { deriveCubeOutputSurfaceNames } from '../CubeOutputSurfaceNames.js';
/** Resolve stable canonical boundary IDs against the slots actually exposed by a node. */
export function buildCubeVersionBoundaryMap(payload, node) {
    const topology = buildCubePayloadTopology(payload);
    const inputIdByName = new Map(topology.inputs.map((boundary) => [boundary.name, boundary.id]));
    const outputNames = deriveCubeOutputSurfaceNames(topology.outputs);
    const outputIdByName = new Map(topology.outputs.map((boundary, index) => [outputNames[index] ?? boundary.name, boundary.id]));
    const inputIdBySlot = mapSlotIds(node.inputs, inputIdByName);
    const outputIdBySlot = mapSlotIds(node.outputs, outputIdByName);
    return {
        inputIdBySlot,
        inputSlotById: invertUnique(inputIdBySlot),
        outputIdBySlot,
        outputSlotById: invertUnique(outputIdBySlot),
    };
}
/** Match host slots to canonical IDs by their authored runtime names. */
function mapSlotIds(slots, idByName) {
    const mapped = new Map();
    slots.forEach((slot, index) => {
        const name = typeof slot.name === 'string' ? slot.name : '';
        const id = idByName.get(name);
        if (id)
            mapped.set(index, id);
    });
    return mapped;
}
/** Invert a slot map while rejecting ambiguous persisted boundary identities. */
function invertUnique(values) {
    const inverted = new Map();
    for (const [slot, id] of values) {
        if (inverted.has(id))
            throw new Error(`Cube boundary identity '${id}' is duplicated.`);
        inverted.set(id, slot);
    }
    return inverted;
}
