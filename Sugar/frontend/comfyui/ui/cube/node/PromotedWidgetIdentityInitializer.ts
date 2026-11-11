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
/** Initialize promoted widget stores after a native node receives its durable id. */

import type { UnknownRecord } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';

/** Reconfigure a SubgraphNode so promoted widget ids include its assigned node id. */
export function initializePromotedWidgetIdentity(node: ComfyNode): void {
  if (node.isSubgraphNode?.() !== true) {
    return;
  }
  const configure = Reflect.get(node, 'configure');
  if (typeof configure !== 'function') {
    throw new TypeError(`Subgraph node '${String(node.id ?? '')}' cannot initialize its widgets.`);
  }
  configure.call(node, {} satisfies UnknownRecord);
}
