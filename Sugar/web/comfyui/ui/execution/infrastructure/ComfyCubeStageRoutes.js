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
/** Decode explicit compiled Cube bindings into stable stage routes independent of renderer state. */
import { isRecord } from '../../types/common.js';
import { isCubeOutputExecutionId } from '../../cube/execution/CubeOutputExecutionIdentity.js';
/** Index only declared Cube sinks, refusing ambiguous shared-instance preview attribution. */
export class ComfyCubeStageRoutes {
    #routes;
    /** Validate transport-owned values before indexing the explicitly bound image producer. */
    constructor(prompt) {
        const routes = [];
        for (const [outputPath, node] of Object.entries(prompt)) {
            if (!isRecord(node) || node.class_type !== 'SugarCubes.CubeOutput')
                continue;
            const inputs = isRecord(node.inputs) ? node.inputs : null;
            const instanceId = inputs?.instance_id;
            const value = inputs?.value;
            if (!isCubeOutputExecutionId(outputPath) ||
                typeof instanceId !== 'string' ||
                !instanceId.trim() ||
                instanceId.length > 4096 ||
                !Array.isArray(value) ||
                value.length !== 2 ||
                typeof value[0] !== 'string' ||
                !value[0].startsWith(`${instanceId}:`) ||
                typeof value[1] !== 'number' ||
                !Number.isSafeInteger(value[1]) ||
                value[1] < 0)
                continue;
            routes.push({ outputPath, boundPath: value[0], instanceId });
        }
        this.#routes = Object.freeze(routes);
    }
    /** Select a unique exposed stage; multiple outputs require an exact bound producer. */
    resolve(executionPath) {
        const candidates = this.#routes.filter((route) => executionPath.startsWith(`${route.instanceId}:`));
        if (candidates.length === 1)
            return candidates[0] ?? null;
        const exact = candidates.filter((route) => route.boundPath === executionPath);
        return exact.length === 1 ? (exact[0] ?? null) : null;
    }
}
