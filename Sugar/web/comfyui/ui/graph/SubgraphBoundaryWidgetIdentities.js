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
/** Identify widget fields supplied by a serialized subgraph input boundary. */
const SUBGRAPH_INPUT_NODE_ID = '-10';
/** Index boundary-supplied widget names by their target node identity. */
export function indexSubgraphBoundaryWidgetNames(subgraph) {
    const boundaryTargets = boundaryLinkTargets(subgraph.links);
    const indexed = new Map();
    for (const node of subgraph.nodes ?? []) {
        const nodeId = graphIdKey(node.id);
        if (!nodeId)
            continue;
        const names = new Set();
        for (const input of node.inputs ?? []) {
            const linkId = graphIdKey(input.link);
            const name = readWidgetName(input);
            if (linkId && name && boundaryTargets.get(linkId) === nodeId)
                names.add(name);
        }
        if (names.size > 0)
            indexed.set(nodeId, names);
    }
    return indexed;
}
/** Map subgraph-input link identities to their exact target nodes. */
function boundaryLinkTargets(links) {
    const targets = new Map();
    for (const link of links ?? []) {
        const originId = graphIdKey(link.origin_id ?? link.origin);
        const linkId = graphIdKey(link.id);
        const targetId = graphIdKey(link.target_id ?? link.target);
        if (originId === SUBGRAPH_INPUT_NODE_ID && linkId && targetId) {
            targets.set(linkId, targetId);
        }
    }
    return targets;
}
/** Read one widget's stable identity from its adjacent serialized input. */
function readWidgetName(input) {
    const widgetName = input.widget?.name;
    const rawName = typeof widgetName === 'string' ? widgetName : input.name;
    return typeof rawName === 'string' ? rawName.trim() : '';
}
/** Normalize Comfy's supported number-or-string graph identity for lookup. */
function graphIdKey(value) {
    return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}
