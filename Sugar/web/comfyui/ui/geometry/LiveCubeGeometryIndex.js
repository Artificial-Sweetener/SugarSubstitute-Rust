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
/** Index live managed cube geometry without interpreting renderer policy. */
import { isRecord } from '../types/common.js';
import { AUTHORED_LAYOUT_KEY, readAuthoredLayoutBaseline, } from './AuthoredLayoutBaseline.js';
/** Resolve managed groups, authored baselines, and their live nodes from one graph. */
export function collectLiveGeometryInstances(graph) {
    const graphNodes = Array.isArray(graph._nodes)
        ? graph._nodes
        : Array.isArray(graph.nodes)
            ? graph.nodes
            : [];
    const groups = Array.isArray(graph._groups)
        ? graph._groups
        : Array.isArray(graph.groups)
            ? graph.groups
            : [];
    const result = [];
    for (const group of groups) {
        const metadata = isRecord(group.properties) && isRecord(group.properties.sugarcubes)
            ? group.properties.sugarcubes
            : null;
        const baseline = readAuthoredLayoutBaseline(metadata?.[AUTHORED_LAYOUT_KEY]);
        if (!metadata || !baseline)
            continue;
        const memberIds = readMemberIds(metadata);
        const nodes = new Map();
        for (const node of graphNodes) {
            if (memberIds.size && !memberIds.has(String(node.id)))
                continue;
            const identity = isRecord(node.properties) && typeof node.properties.sugarcubes_symbol === 'string'
                ? node.properties.sugarcubes_symbol.trim()
                : '';
            if (identity && baseline.entries[identity])
                nodes.set(identity, node);
        }
        if (nodes.size)
            result.push({ baseline, group, metadata, nodes });
    }
    return result;
}
function readMemberIds(metadata) {
    const ids = new Set();
    if (Array.isArray(metadata.nodes)) {
        for (const value of metadata.nodes)
            ids.add(String(value));
    }
    if (isRecord(metadata.markers)) {
        for (const values of [metadata.markers.inputs, metadata.markers.outputs]) {
            if (Array.isArray(values))
                for (const value of values)
                    ids.add(String(value));
        }
    }
    return ids;
}
