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
/** Adapt missing-node container hints before Comfy surfaces them. */
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';
/** Own Cube-specific missing-node labels at Comfy's after-configure hook. */
export class CubeMissingNodeLabelAdapter {
    #nodes;
    /** Bind the live root-Cube catalog used by execution path prefixes. */
    constructor(nodes) {
        this.#nodes = nodes;
    }
    /** Replace only hints whose execution path begins at a known Cube instance. */
    adapt(missingNodes) {
        const byNodeId = new Map(this.#nodes.list().map((node) => [String(node.id), node]));
        let changed = 0;
        for (const entry of missingNodes) {
            if (!isRecord(entry))
                continue;
            const executionId = typeof entry.nodeId === 'string' ? entry.nodeId : '';
            const rootId = executionId.split(':')[0] ?? '';
            const node = byNodeId.get(rootId);
            if (!node)
                continue;
            const identity = requireCubeIdentity(node);
            const alias = identity.default_alias;
            const title = typeof alias === 'string' && alias.trim() ? alias.trim() : node.title;
            entry.hint = `in Cube '${title || 'Untitled Cube'}'`;
            changed += 1;
        }
        return changed;
    }
}
