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
/** Resolve complete node execution modes alongside shared eligible widget configuration. */
import { canLinkNode } from './RelationResolver.js';
/** Preserve dormant follower activation and resolve a valid master without depending on graph links. */
export function resolveLinkedNodeModes(nodes, state) {
    const byId = new Map(nodes.map((node) => [node.id, node]));
    const result = new Map();
    for (const node of nodes) {
        let current = node, sourceId = state.nodeLinks[node.id];
        const visited = new Set([node.id]);
        let valid = true;
        while (sourceId) {
            const source = byId.get(sourceId);
            if (!source || visited.has(sourceId) || !canLinkNode(current, source, nodes, state)) {
                valid = false;
                break;
            }
            visited.add(sourceId);
            current = source;
            sourceId = state.nodeLinks[sourceId];
        }
        if (!valid || current.id === node.id) {
            result.set(node.id, state.localNodeModes[node.id] ?? node.mode ?? 0);
            continue;
        }
        state.localNodeModes[node.id] ??= node.mode ?? 0;
        result.set(node.id, state.localNodeModes[current.id] ?? current.mode ?? 0);
    }
    return result;
}
