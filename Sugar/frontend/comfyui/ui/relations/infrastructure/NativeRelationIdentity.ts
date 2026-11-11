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
/** Validate stable node and connection identities before whole-node sharing is offered. */
import { collectGraphLinks } from '../../graph/GraphQuery.js';
import type { ComfyGraph, ComfyLink, ComfyNode } from '../../types/graph.js';

/** Cache connection inventories for one topology discovery operation. */
export class NativeRelationIdentity {
  readonly #links = new Map<ComfyGraph, readonly ComfyLink[]>();

  /** Match authored symbols and noneditable connection shapes rather than live numeric node IDs. */
  read(node: ComfyNode, symbol: string, editableNames: readonly string[]): string | null {
    const signature: Array<readonly [string, string, number]> = [];
    for (const input of node.inputs ?? []) {
      if (
        input.link === null ||
        input.link === undefined ||
        editableNames.includes(input.name ?? '')
      )
        continue;
      const graph = node.graph;
      if (!graph) return null;
      let links = this.#links.get(graph);
      if (!links) {
        links = collectGraphLinks(graph);
        this.#links.set(graph, links);
      }
      const link =
        graph.getLink?.(input.link) ?? links.find((link) => String(link.id) === String(input.link));
      const sourceId = link?.origin_id ?? link?.origin;
      const source = (graph._nodes ?? graph.nodes ?? []).find(
        (candidate) => String(candidate.id) === String(sourceId),
      );
      if (!source || typeof link?.origin_slot !== 'number' || !Number.isInteger(link.origin_slot))
        return null;
      const sourceSymbol =
        typeof source.properties?.sugarcubes_symbol === 'string'
          ? source.properties.sugarcubes_symbol
          : String(source.id);
      signature.push([input.name ?? '', sourceSymbol, link.origin_slot]);
    }
    signature.sort((left, right) => left[0].localeCompare(right[0]));
    return JSON.stringify([symbol, signature]);
  }
}
