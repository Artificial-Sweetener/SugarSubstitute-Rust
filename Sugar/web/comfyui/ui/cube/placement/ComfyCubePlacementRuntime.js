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
/** Compose root placement policy, inventory, and defensive Comfy mutation integration. */
import { isRecord } from '../../types/common.js';
import { CubeGraphInventory } from '../node/CubeGraphInventory.js';
import { ComfyCubeGraphScope } from './ComfyCubeGraphScope.js';
import { ComfyCubeHostMutationGuard, } from './ComfyCubeHostMutationGuard.js';
/** Validate dynamic host capabilities and return focused graph-bound owners. */
export function createComfyCubePlacementRuntime(options) {
    const graphScope = new ComfyCubeGraphScope(options.rootGraph, () => options.canvas.graph);
    const graphInventory = new CubeGraphInventory(options.rootGraph);
    const hostPlacementGuard = new ComfyCubeHostMutationGuard({
        rootGraph: requirePlacementGraph(options.rootGraph),
        canvas: requirePlacementCanvas(options.canvas),
        policy: graphScope.policy(),
        reportError(summary, detail) {
            options.logger.warn('SugarCubes rejected a non-root Cube placement.', { summary, detail });
            options.feedback?.push?.('error', summary, detail);
        },
    });
    return { graphScope, graphInventory, hostPlacementGuard };
}
/** Require the graph mutation seam guarded against host-controlled nested placement. */
function requirePlacementGraph(value) {
    if (typeof value.add !== 'function')
        throw new TypeError('Comfy LGraph.add is unavailable.');
    return value;
}
/** Require the clipboard mutation seam shared by native paste and clone operations. */
function requirePlacementCanvas(value) {
    if (!isRecord(value) || typeof value._deserializeItems !== 'function') {
        throw new TypeError('Comfy LGraphCanvas._deserializeItems is unavailable.');
    }
    return value;
}
