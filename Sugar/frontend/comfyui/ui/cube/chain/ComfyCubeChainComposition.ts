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
/** Bind the Cube chain to the live root graph and tell the stack surfaces when it may change. */
import type { ComfyGraph } from '../../types/graph.js';
import type { ProximityMatch } from '../../overlays/proximity/ProximityModel.js';
import type { CanvasGraphChangeSource } from '../../surface/ComfyCanvasGraphChangeAdapter.js';
import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
import { CubeNodeSwapCoordinator } from '../node/CubeNodeSwapCoordinator.js';
import type { CubeNodeCatalog } from '../node/CubeNodeCatalog.js';
import type { CubeChainChange } from './ComfyCubeChainCommitHost.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { CubeChainService } from './CubeChainService.js';

interface ComfyCubeChainOptions {
  readonly graph: ComfyGraph;
  readonly nodes: CubeNodeCatalog;
  readonly commit: { commit(change: CubeChainChange): CubeNode[] };
  readonly graphChanges: CanvasGraphChangeSource;
  isCurrentRoot(): boolean;
  matches(): readonly ProximityMatch[];
  accepts(outputType: unknown, inputType: unknown): boolean;
  reportError(error: unknown): void;
}

/** Compose the chain with the graph events that can move a Cube in the stack. */
export function createComfyCubeChain(options: ComfyCubeChainOptions) {
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
  const schedule = (): void => {
    if (queued || disposed) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      if (!disposed) service.changed();
    });
  };
  const observer = new NativeSubgraphChangeObserver(options.graph, schedule);
  const releaseNavigation = options.graphChanges.subscribe(schedule);
  const releaseCatalog = options.nodes.subscribe(schedule);
  return {
    service,
    swap: new CubeNodeSwapCoordinator({ chain: service, reportError: options.reportError }),
    dispose(): void {
      disposed = true;
      observer.dispose();
      releaseNavigation();
      releaseCatalog();
    },
  };
}
