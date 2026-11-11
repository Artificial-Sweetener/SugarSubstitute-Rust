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
/** Observe one Cube face's native topology and scoped field presentation through their existing owners. */
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { NativeSubgraphChangeObserver } from './NativeSubgraphChangeObserver.js';
import { subscribeNativeNodeCardRefresh } from './NativeNodeCardRefresh.js';

/** Keep card visibility responsive to native field leases without rediscovering unrelated graph nodes. */
export class CubeSurfaceChangeObserver {
  readonly #topology: NativeSubgraphChangeObserver;
  readonly #stop: () => void;
  /** Watch exact graph and field owners for this mounted Cube, including its exposed boundary controls. */
  constructor(document: Document, node: CubeNode, changed: () => void) {
    this.#topology = new NativeSubgraphChangeObserver(node.subgraph, changed);
    this.#stop = subscribeNativeNodeCardRefresh(document, [node, ...node.subgraph._nodes], changed);
  }
  /** Release both subscriptions before the native face is replaced or unmounted. */
  dispose(): void {
    this.#topology.dispose();
    this.#stop();
  }
}
