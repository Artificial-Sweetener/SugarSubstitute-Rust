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
/** Build the opt-in SugarCubes host debugging surface. */
import { computeInnerBounds } from '../graph/CubeBounds.js';
/** Create the existing debugging API from authoritative UI owners. */
export function createSugarCubesDebugApi({ ui, app }) {
    return {
        getDirtyState(instanceId) {
            return ui.dirtyManager.getDebugState(instanceId);
        },
        workflow: {
            classification(instanceId) {
                if (typeof instanceId !== 'string')
                    return null;
                const result = ui.workflowLibraryState.read(instanceId);
                return result ? { ...result, permittedOperations: [...result.permittedOperations] } : null;
            },
        },
        execution: {
            proximity() {
                return ui.overlayManager.proximity.executionDebugState();
            },
        },
        bounds: {
            get(instanceId) {
                if (!instanceId)
                    return null;
                const graph = app?.graph || null;
                const index = ui.containmentService?.buildIndex?.(graph) || null;
                const entry = index?.instanceById?.get?.(String(instanceId)) || null;
                if (!entry?.metadata?.bounds)
                    return null;
                return {
                    bounds: entry.metadata.bounds,
                    inner: computeInnerBounds(entry.metadata.bounds),
                };
            },
            reconcile(instanceId) {
                const graph = app?.graph || null;
                if (!graph || !ui.boundsReconciler)
                    return { changed: [] };
                const result = ui.boundsReconciler.reconcileAll({ graph });
                const changed = Array.from(result.changed || []);
                if (instanceId && !changed.includes(String(instanceId)))
                    return { changed: [] };
                return { changed };
            },
            resolveCollisions(instanceId) {
                const graph = app?.graph || null;
                if (!graph || !ui.collisionService || !instanceId)
                    return { moved: false };
                const index = ui.containmentService?.buildIndex?.(graph) || null;
                return ui.collisionService.resolveCollisions({
                    graph,
                    activeInstanceId: String(instanceId),
                    index,
                });
            },
        },
        layout: {
            service: ui.layoutService,
            appendCube: (options) => ui.layoutService.appendCube(options),
            insertBetween: (options) => ui.layoutService.insertBetween(options),
            insertBefore: (options) => ui.layoutService.insertBefore(options),
            swapOrder: (options) => ui.layoutService.swapOrder(options),
            replaceCube: (options) => ui.layoutService.replaceCube(options),
        },
    };
}
