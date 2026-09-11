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
/** Rehydrate saved Cube widgets before Comfy applies positional values. */

import { rebindSubgraphWidgetValues } from '../graph/SubgraphWidgetValueRebinder.js';
import { isRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';

const CUBE_KINDS = new Set(['cube', 'cube_draft']);
const WIDGET_INPUT_TYPES = new Set([
  'BOOLEAN',
  'COMBO',
  'FLOAT',
  'INT',
  'LIST',
  'NUMBER',
  'STRING',
  'TEXT',
]);

type LiveNodeFactory = (type: string) => ComfyNode | null;

/** Own preconfiguration-time Cube widget migration into the current node schema. */
export class CubeSerializedWidgetRehydrator {
  readonly #createNode: LiveNodeFactory;

  /** Bind the current Comfy node factory used to discover live widget layouts. */
  constructor(createNode: LiveNodeFactory) {
    this.#createNode = createNode;
  }

  /** Rebind every Cube-owned serialized subgraph before native configuration. */
  prepare(workflow: unknown): number {
    if (!isRecord(workflow) || !isRecord(workflow.definitions)) return 0;
    const definitions = workflow.definitions.subgraphs;
    if (!Array.isArray(definitions)) return 0;

    const byId = new Map<string, Record<string, unknown>>();
    for (const definition of definitions) {
      if (!isRecord(definition)) continue;
      const id = readString(definition.id);
      if (id) byId.set(id, definition);
    }

    const pending = definitions.filter(isCubeDefinition).map((definition) => ({
      definition,
      historicalWidgetNames: indexHistoricalWidgetNames(definition),
    }));
    const visited = new Set<string>();
    let rehydrated = 0;
    while (pending.length > 0) {
      const entry = pending.shift();
      if (!entry) continue;
      const { definition, historicalWidgetNames } = entry;
      const id = readString(definition.id);
      if (!id || visited.has(id)) continue;
      visited.add(id);

      rebindSubgraphWidgetValues(definition, (type) => (type ? this.#createNode(type) : null), {
        unavailableNode: 'preserve',
        historicalWidgetNames: (node) =>
          historicalWidgetNames.get(readString(node.type ?? node.class_type)) ?? null,
        unidentifiedValues: 'discard',
      });
      rehydrated += 1;
      for (const node of Array.isArray(definition.nodes) ? definition.nodes : []) {
        if (!isRecord(node)) continue;
        const nested = byId.get(readString(node.type));
        if (nested && !visited.has(readString(nested.id))) {
          pending.push({ definition: nested, historicalWidgetNames });
        }
      }
    }
    return rehydrated;
  }
}

/** Index the widget order embedded with the Cube's original node definitions. */
function indexHistoricalWidgetNames(
  definition: Record<string, unknown>,
): Map<string, readonly string[]> {
  const result = new Map<string, readonly string[]>();
  if (!isRecord(definition.extra) || !isRecord(definition.extra.sugarcubes_cube)) return result;
  const definitions = definition.extra.sugarcubes_cube.definitions;
  if (!isRecord(definitions)) return result;
  for (const [nodeType, nodeDefinition] of Object.entries(definitions)) {
    if (!isRecord(nodeDefinition)) continue;
    const names = readWidgetInputNames(nodeDefinition);
    if (names.length > 0) result.set(nodeType, names);
  }
  return result;
}

/** Read widget-backed input names in the source node definition's declared order. */
function readWidgetInputNames(definition: Record<string, unknown>): string[] {
  if (!isRecord(definition.input)) return [];
  const inputOrder = isRecord(definition.input_order) ? definition.input_order : {};
  const names: string[] = [];
  for (const sectionName of ['required', 'optional']) {
    const fields = definition.input[sectionName];
    if (!isRecord(fields)) continue;
    const declaredOrder = Array.isArray(inputOrder[sectionName])
      ? inputOrder[sectionName].filter((name): name is string => typeof name === 'string')
      : [];
    const orderedNames = [
      ...declaredOrder,
      ...Object.keys(fields).filter((name) => !declaredOrder.includes(name)),
    ];
    for (const name of orderedNames) {
      if (!names.includes(name) && isWidgetFieldSpec(fields[name])) names.push(name);
    }
  }
  return names;
}

/** Identify one compact Comfy field specification that was represented by a widget. */
function isWidgetFieldSpec(value: unknown): boolean {
  if (!Array.isArray(value) || value.length === 0) return false;
  const metadata = isRecord(value[1]) ? value[1] : null;
  if (metadata?.forceInput === true) return false;
  const type = value[0];
  if (Array.isArray(type)) return true;
  return typeof type === 'string' && WIDGET_INPUT_TYPES.has(type.toUpperCase());
}

/** Identify a serialized SugarCube definition without touching ordinary subgraphs. */
function isCubeDefinition(value: unknown): value is Record<string, unknown> {
  return (
    isRecord(value) &&
    isRecord(value.extra) &&
    CUBE_KINDS.has(readString(value.extra.sugarcubes_kind))
  );
}

/** Normalize one optional serialized identity. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
