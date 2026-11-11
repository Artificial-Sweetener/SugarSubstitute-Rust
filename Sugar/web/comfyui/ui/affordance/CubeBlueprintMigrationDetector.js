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
/** Detect legacy Sugar-marked Blueprint placements without mutating user data. */
import { isSugarMarkedBlueprintNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';
/** Surface recoverable legacy Blueprint data and exclude it from live Cube ownership. */
export class CubeBlueprintMigrationDetector {
    #feedback;
    #logger;
    #reported = new Set();
    /** Bind non-destructive migration feedback. */
    constructor(options) {
        this.#feedback = options.feedback ?? null;
        this.#logger = options.logger;
    }
    /** Report newly discovered legacy placements once per live graph object. */
    scan(graph) {
        if (!isRecord(graph) || !Array.isArray(graph._nodes))
            return 0;
        const discovered = graph._nodes.filter((node) => isSugarMarkedBlueprintNode(node) && !this.#reported.has(node));
        for (const node of discovered)
            this.#reported.add(node);
        if (discovered.length === 0)
            return 0;
        this.#logger.warn('SugarCubes detected legacy Sugar-marked Subgraph Blueprint placements.', {
            count: discovered.length,
        });
        this.#feedback?.push?.('warn', 'Legacy SugarCube Blueprint detected', `${discovered.length} legacy Blueprint placement${discovered.length === 1 ? '' : 's'} remains ordinary Subgraph data. Re-import the original .cube to restore versioning and sync.`);
        return discovered.length;
    }
}
