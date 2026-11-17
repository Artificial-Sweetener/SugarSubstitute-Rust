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
/** Keep the seeds of a workflow rebuilt from a finished image as the image recorded them. */
import { isComfySeedModeControl } from '../graph/ComfySeedMode.js';
import { isRecord } from '../types/common.js';
/**
 * Set every seed of the named Cube instances to stay fixed.
 *
 * A Cube document records a seed's value and nothing about what happens to it after a run, so
 * a rebuilt Cube takes Comfy's default and rolls a new seed as soon as it is queued. A workflow
 * read from a finished image is opened to get that image again, so its seeds stay as recorded
 * until the person chooses otherwise.
 *
 * @returns How many seed controls were changed.
 */
export function fixImportedSeeds(root, instanceIds) {
    const wanted = new Set(instanceIds);
    let changed = 0;
    const fix = (graph, inside, depth) => {
        if (!isRecord(graph) || depth > 32)
            return;
        const nodes = graph._nodes ?? graph.nodes;
        if (!Array.isArray(nodes))
            return;
        for (const node of nodes) {
            if (!isRecord(node))
                continue;
            const cube = isRecord(node.properties) ? node.properties.sugarcubes_cube : null;
            const owned = inside ||
                (isRecord(cube) && typeof cube.instance_id === 'string' && wanted.has(cube.instance_id));
            if (owned && Array.isArray(node.widgets))
                for (const widget of node.widgets)
                    if (isComfySeedModeControl(widget) && widget.value !== 'fixed') {
                        widget.value = 'fixed';
                        changed += 1;
                    }
            if (owned)
                fix(node.subgraph, true, depth + 1);
        }
    };
    fix(root, false, 0);
    return changed;
}
