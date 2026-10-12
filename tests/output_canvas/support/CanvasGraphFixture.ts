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
/** Model the public graph mutation/serialization boundaries used by source inventory. */

export interface CanvasNodeFixture {
  id: number;
  title: string;
  type: string;
  properties?: { sugarcubes_kind: string };
  subgraph?: CanvasGraphFixture;
}

/** Count serialization separately from scoped native changes. */
export class CanvasGraphFixture {
  readonly _nodes: CanvasNodeFixture[] = [];
  readonly outputs: { name: string }[] = [];
  name = 'Nested';
  serializations = 0;
  onAfterChange: ((graph: unknown, info?: unknown) => void) | null = null;

  /** Add a native node through the observable topology boundary. */
  add(node: CanvasNodeFixture): CanvasNodeFixture {
    this._nodes.push(node);
    return node;
  }

  /** Remove a native node through the observable topology boundary. */
  remove(node: CanvasNodeFixture): void {
    const index = this._nodes.indexOf(node);
    if (index >= 0) this._nodes.splice(index, 1);
  }

  /** Emit value, title or generic graph completion with an optional affected handle. */
  afterChange(info?: unknown): void {
    this.onAfterChange?.(this, info);
  }

  /** Return a complete serialized workflow without exporting live handles. */
  serialize(): unknown {
    this.serializations++;
    return {
      name: this.name,
      outputs: this.outputs.map((output) => ({ ...output })),
      nodes: this._nodes.map((node) => ({
        id: node.id,
        title: node.title,
        type: node.type,
        ...(node.properties ? { properties: { ...node.properties } } : {}),
        ...(node.subgraph ? { subgraph: node.subgraph.serialize() } : {}),
      })),
    };
  }
}
