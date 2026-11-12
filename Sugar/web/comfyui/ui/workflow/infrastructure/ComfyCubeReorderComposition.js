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
/** Compose canonical graph reordering with native lifecycle and face-action adapters. */
import { CubeGraphReorderService } from '../application/CubeGraphReorderService.js';
import { ComfyCubeGraphReorderHost } from './ComfyCubeGraphReorderHost.js';
import { CubeNodeSwapCoordinator } from '../../cube/node/CubeNodeSwapCoordinator.js';
import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
/** Bind refreshes to graph events while retaining one permission and geometry authority. */
export function createComfyCubeReorder(options) {
    const host = new ComfyCubeGraphReorderHost({
        graph: options.graph,
        history: options.history,
        isCurrentRoot: options.isCurrentRoot,
        reconcileOrder: options.reconcileOrder,
        onCommitted: (changed) => {
            options.nodes.batch(() => {
                for (const node of options.nodes.list())
                    if (changed.includes(node))
                        options.nodes.changed(node);
            });
        },
    });
    const service = new CubeGraphReorderService({ gateway: options.gateway, host });
    let disposed = false;
    let queued = false;
    const schedule = () => {
        if (queued || disposed)
            return;
        queued = true;
        queueMicrotask(() => {
            queued = false;
            if (!disposed)
                void service.refresh().catch(options.reportError);
        });
    };
    const graph = options.graph;
    if (typeof graph.add !== 'function' || typeof graph.remove !== 'function')
        throw new TypeError('Comfy workflow topology methods are unavailable for Cube reorder.');
    const observer = new NativeSubgraphChangeObserver(graph, () => {
        host.invalidateSnapshot();
        service.invalidate();
        schedule();
    });
    const releaseNavigation = options.graphChanges.subscribe(() => {
        host.invalidateScope();
        service.invalidate();
        schedule();
    });
    const releaseCatalog = options.nodes.subscribe(schedule);
    const releasePermissions = service.subscribe(() => {
        options.history.setDirtyCanvas(true, true);
    });
    schedule();
    return {
        service,
        snapshots: host,
        swap: new CubeNodeSwapCoordinator({ reorder: service, reportError: options.reportError }),
        dispose() {
            disposed = true;
            host.dispose();
            observer.dispose();
            releaseNavigation();
            releaseCatalog();
            releasePermissions();
            service.invalidate();
        },
    };
}
