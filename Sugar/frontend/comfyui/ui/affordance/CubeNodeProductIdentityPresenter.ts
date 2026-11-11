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
/** Present marked native SubgraphNode instances as SugarCubes to generic host UI. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';

/** Own the instance-local product type exposed to Comfy menus and inspectors. */
export class CubeNodeProductIdentityPresenter {
  readonly #nodes: CubeNodeCatalog;
  readonly #originalDescriptors = new Map<CubeNode, PropertyDescriptor | undefined>();
  readonly #unsubscribe: () => void;

  /** Follow the live Cube catalog without changing structural Subgraph identity. */
  constructor(nodes: CubeNodeCatalog) {
    this.#nodes = nodes;
    this.#unsubscribe = nodes.subscribe(() => this.#refresh());
    this.#refresh();
  }

  /** Restore instance descriptors when Comfy replaces the workflow graph. */
  dispose(): void {
    this.#unsubscribe();
    for (const [node, descriptor] of this.#originalDescriptors) {
      if (descriptor) Object.defineProperty(node, 'displayType', descriptor);
      else Reflect.deleteProperty(node, 'displayType');
    }
    this.#originalDescriptors.clear();
  }

  /** Decorate each newly discovered Cube exactly once. */
  #refresh(): void {
    for (const node of this.#nodes.list()) {
      if (this.#originalDescriptors.has(node)) continue;
      this.#originalDescriptors.set(node, Object.getOwnPropertyDescriptor(node, 'displayType'));
      Object.defineProperty(node, 'displayType', {
        configurable: true,
        enumerable: false,
        get: () => 'SugarCube',
      });
    }
  }
}
