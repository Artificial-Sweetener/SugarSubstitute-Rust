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
/** Detect native graph membership/exposure changes without serializing scoped value updates. */

import { isRecord, type UnknownRecord } from '../../types/common.js';

interface NodeFact {
  readonly handle: UnknownRecord;
  readonly id: unknown;
  readonly type: unknown;
  readonly order: unknown;
  readonly subgraph: unknown;
  readonly cube: unknown;
}

/** Snapshot only facts that can alter declared source identity or authored source ordering. */
export class CanvasGraphSourceTopology {
  readonly #nodes: readonly NodeFact[];
  readonly #outputs: readonly unknown[];

  /** Capture existing native handles at an explicit discovery boundary. */
  constructor(private readonly graph: UnknownRecord) {
    this.#nodes = nodes(graph)
      .filter(isRecord)
      .map((node) => ({
        handle: node,
        id: node.id,
        type: node.type,
        order: node.order,
        subgraph: node.subgraph,
        cube: isRecord(node.properties) ? node.properties.sugarcubes_kind : undefined,
      }));
    this.#outputs = Array.isArray(graph.outputs) ? [...graph.outputs] : [];
  }

  /** Recognize undo, exposure and instance replacement through a generic graph completion callback. */
  changed(): boolean {
    const current = nodes(this.graph).filter(isRecord);
    const outputs: readonly unknown[] = Array.isArray(this.graph.outputs) ? this.graph.outputs : [];
    return (
      current.length !== this.#nodes.length ||
      current.some((node, index) => {
        const previous = this.#nodes[index];
        return (
          !previous ||
          node !== previous.handle ||
          node.id !== previous.id ||
          node.type !== previous.type ||
          node.order !== previous.order ||
          node.subgraph !== previous.subgraph ||
          (isRecord(node.properties) ? node.properties.sugarcubes_kind : undefined) !==
            previous.cube
        );
      }) ||
      outputs.length !== this.#outputs.length ||
      outputs.some((output, index) => output !== this.#outputs[index])
    );
  }
}

/** Read a native node list without traversing nested graphs or invoking serialization. */
function nodes(graph: UnknownRecord): readonly unknown[] {
  return Array.isArray(graph._nodes) ? graph._nodes : Array.isArray(graph.nodes) ? graph.nodes : [];
}
