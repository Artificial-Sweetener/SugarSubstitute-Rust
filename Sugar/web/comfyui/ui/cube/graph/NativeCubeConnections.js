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
/** Capture and restore native connections through the shared validated root mutation boundary. */
import { isRecord } from '../../types/common.js';
import { isNativeGraphNode } from '../ComfyCubeRuntimeGraphBoundary.js';
/** Validate one native link before exposing its endpoint and slot semantics. */
export function readNativeCubeConnection(graph, id) {
    const link = graph.getLink(id);
    if (!link)
        throw new Error(`Cube connection '${String(id)}' is unavailable.`);
    const sourceId = link.origin_id ?? link.origin;
    const targetId = link.target_id ?? link.target;
    if (sourceId === null || sourceId === undefined || targetId === null || targetId === undefined)
        throw new Error('Cube connection has an unavailable native endpoint.');
    return {
        source: requireEndpoint(graph.getNodeById(sourceId)),
        target: requireEndpoint(graph.getNodeById(targetId)),
        sourceSlot: requireSlot(link.origin_slot),
        targetSlot: requireSlot(link.target_slot),
    };
}
/** Capture each connection touching a removed Cube once, including links between removed Cubes. */
export function captureNativeCubeConnections(graph, nodes) {
    const ids = new Set();
    for (const node of nodes) {
        for (const input of node.inputs)
            if (input.link !== null && input.link !== undefined)
                ids.add(input.link);
        for (const output of node.outputs)
            for (const id of output.links ?? [])
                ids.add(id);
    }
    return [...ids].map((id) => readNativeCubeConnection(graph, id));
}
/** Restore only disconnected links through native APIs without replacing retained controls or nodes. */
export function restoreNativeCubeConnections(graph, connections) {
    for (const connection of connections) {
        const id = connection.target.inputs?.[connection.targetSlot]?.link;
        const current = id === null || id === undefined ? null : graph.getLink(id);
        if (current &&
            (current.origin_id ?? current.origin) === connection.source.id &&
            current.origin_slot === connection.sourceSlot)
            continue;
        connection.source.connect(connection.sourceSlot, connection.target, connection.targetSlot);
        const restoredId = connection.target.inputs?.[connection.targetSlot]?.link;
        const restored = restoredId === null || restoredId === undefined ? null : graph.getLink(restoredId);
        if (!restored ||
            (restored.origin_id ?? restored.origin) !== connection.source.id ||
            restored.origin_slot !== connection.sourceSlot ||
            (restored.target_id ?? restored.target) !== connection.target.id ||
            restored.target_slot !== connection.targetSlot)
            throw new Error('Native Cube connection could not be restored.');
    }
}
/** Narrow dynamic native endpoints before their callable crosses the adapter boundary. */
function requireEndpoint(value) {
    if (!isRecord(value) ||
        (typeof value.id !== 'string' && typeof value.id !== 'number') ||
        !isNativeGraphNode(value))
        throw new Error('Cube connection endpoint is unavailable.');
    return value;
}
/** Require a finite native slot before mutation or compensation begins. */
function requireSlot(value) {
    if (typeof value !== 'number' || !Number.isInteger(value) || value < 0)
        throw new Error('Cube connection has an invalid native slot.');
    return value;
}
