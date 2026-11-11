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
/** Restore extracted legacy Cube records through Comfy's native subgraph loader. */

import { isRecord } from '../../types/common.js';
import type { BuiltCubeGraph, CubeGraphBuilderHost } from '../ComfyCubeGraphBuilder.js';
import type { LegacyCubeDefinitionSerializer } from './LegacyCubeDefinitionSerializer.js';
import type { LegacyCubePlan } from './LegacyCubeWorkflowExtractor.js';
import { rebindSubgraphWidgetValues } from '../../graph/SubgraphWidgetValueRebinder.js';

/** Own native graph creation for one already-extracted legacy Cube plan. */
export class LegacyCubeGraphBuilder {
  readonly #host: CubeGraphBuilderHost;
  readonly #serializer: LegacyCubeDefinitionSerializer;

  /** Bind the native graph boundary and pure legacy serializer. */
  constructor(host: CubeGraphBuilderHost, serializer: LegacyCubeDefinitionSerializer) {
    this.#host = host;
    this.#serializer = serializer;
  }

  /** Create one configured native subgraph containing the original real nodes. */
  build(plan: LegacyCubePlan): BuiltCubeGraph {
    const definition = this.#serializer.serialize(plan, this.#host.createUuid());
    rebindSubgraphWidgetValues(definition, (type) => (type ? this.#host.createNode(type) : null));
    const subgraph = this.#host.rootGraph.createSubgraph(definition);
    subgraph.configure(definition);
    if (subgraph._nodes.length !== plan.nodes.length) {
      throw new Error(
        `Comfy restored ${subgraph._nodes.length} of ${plan.nodes.length} internal nodes.`,
      );
    }
    const nodesBySymbol = new Map();
    const warnings: string[] = [];
    for (const node of subgraph._nodes) {
      const properties = isRecord(node.properties) ? node.properties : {};
      const symbol = readString(properties.sugarcubes_symbol);
      if (symbol) nodesBySymbol.set(symbol, node);
      else
        warnings.push(
          `Migrated Cube '${plan.title}' contains node '${String(node.id)}' without a stable symbol.`,
        );
    }
    return { subgraph, nodesBySymbol, warnings };
  }
}

/** Read one non-empty persisted symbol. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
