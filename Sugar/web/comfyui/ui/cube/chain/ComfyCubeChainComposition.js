import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
import { CubeNodeSwapCoordinator } from '../node/CubeNodeSwapCoordinator.js';
import { CubeChainService } from './CubeChainService.js';
/** Compose the chain with the graph events that can move a Cube in the stack. */
export function createComfyCubeChain(options) {
    const service = new CubeChainService({
        graph: options.graph,
        nodes: options.nodes,
        isCurrentRoot: options.isCurrentRoot,
        matches: options.matches,
        accepts: options.accepts,
        commit: options.commit,
    });
    let disposed = false;
    let queued = false;
    // Positions, links and Cubes change in bursts; the stack reads the graph once they settle.
    const schedule = () => {
        if (queued || disposed)
            return;
        queued = true;
        queueMicrotask(() => {
            queued = false;
            if (!disposed)
                service.changed();
        });
    };
    const observer = new NativeSubgraphChangeObserver(options.graph, schedule);
    const releaseNavigation = options.graphChanges.subscribe(schedule);
    const releaseCatalog = options.nodes.subscribe(schedule);
    return {
        service,
        swap: new CubeNodeSwapCoordinator({ chain: service, reportError: options.reportError }),
        dispose() {
            disposed = true;
            observer.dispose();
            releaseNavigation();
            releaseCatalog();
        },
    };
}
