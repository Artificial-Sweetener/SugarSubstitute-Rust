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
/** Remap embedded native-subgraph identities for isolated version staging. */

import { readImportPayload, type ImportPayload } from '../../import/PlacementPayload.js';
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';

export interface RemappedCubeSubgraphs {
  payload: ImportPayload;
  definitionIds: readonly string[];
}

/** Clone one payload and replace embedded definition references with fresh runtime IDs. */
export function remapCubeEmbeddedSubgraphs(
  payload: ImportPayload,
  createId: () => string,
): RemappedCubeSubgraphs {
  const cloned = clonePayload(payload);
  const definitions = Array.isArray(cloned.subgraphs) ? cloned.subgraphs : [];
  const idMap = new Map<string, string>();
  for (const definition of definitions) {
    const id = readString(definition.id);
    if (!id) continue;
    if (idMap.has(id)) throw new Error(`Embedded subgraph identity '${id}' is duplicated.`);
    idMap.set(id, requireFreshId(createId()));
  }
  cloned.nodes = (cloned.nodes ?? []).map((node) => remapNodeType(node, idMap));
  cloned.subgraphs = definitions.map((definition) => remapDefinition(definition, idMap));
  return { payload: cloned, definitionIds: [...idMap.values()] };
}

/** Remap one embedded definition and every nested wrapper reference it owns. */
function remapDefinition(
  definition: UnknownRecord,
  idMap: ReadonlyMap<string, string>,
): UnknownRecord {
  const id = readString(definition.id);
  const nodes = Array.isArray(definition.nodes)
    ? definition.nodes.map((node) => (isRecord(node) ? remapNodeType(node, idMap) : node))
    : definition.nodes;
  return {
    ...definition,
    ...(idMap.get(id) ? { id: idMap.get(id) } : {}),
    ...(Array.isArray(nodes) ? { nodes } : {}),
  };
}

/** Rewrite both prepared and serialized Comfy node type fields. */
function remapNodeType<T extends UnknownRecord>(node: T, idMap: ReadonlyMap<string, string>): T {
  const classType = readString(node.class_type);
  const type = readString(node.type);
  return {
    ...node,
    ...(idMap.get(classType) ? { class_type: idMap.get(classType) } : {}),
    ...(idMap.get(type) ? { type: idMap.get(type) } : {}),
  };
}

/** Clone through the persisted JSON domain before changing runtime-only identities. */
function clonePayload(payload: ImportPayload): ImportPayload {
  const decoded: unknown = JSON.parse(JSON.stringify(payload));
  const cloned = readImportPayload(decoded);
  if (!cloned) throw new Error('Cube payload could not be cloned for version staging.');
  return cloned;
}

/** Require a collision-resistant generated runtime identifier. */
function requireFreshId(value: string): string {
  const id = value.trim();
  if (!id) throw new Error('Embedded subgraph runtime identity is required.');
  return id;
}

/** Read one trimmed persisted identifier. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
