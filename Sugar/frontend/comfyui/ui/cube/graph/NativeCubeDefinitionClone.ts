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
/** Clone serialized native contents with isolated runtime node identities and unchanged Cube symbols. */
import type { UnknownRecord } from '../../types/common.js';
import { isRecord } from '../../types/common.js';
import type { NativeCubeSubgraph } from '../ComfyCubeGraphBuilder.js';

/** Detach native widget-store identities while preserving connections, literal values and symbols. */
export function cloneNativeCubeDefinition(
  original: Pick<NativeCubeSubgraph, 'asSerialisable'>,
  createId: () => string,
): UnknownRecord {
  if (!original.asSerialisable)
    throw new TypeError('Cube definition serialization is unavailable.');
  const data: unknown = JSON.parse(JSON.stringify(original.asSerialisable()));
  if (!isRecord(data) || !Array.isArray(data.nodes))
    throw new TypeError('Cube definition serialization must contain nodes.');
  const ids = new Map<string, string>();
  const reserved = new Set<string>();
  for (const node of data.nodes) {
    if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number'))
      throw new TypeError('Cube definition contains an invalid node identity.');
    if (reserved.has(String(node.id)))
      throw new Error('Cube definition node identities are duplicated.');
    reserved.add(String(node.id));
  }
  for (const node of data.nodes) {
    if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number'))
      throw new TypeError('Cube definition contains an invalid node identity.');
    const id = createId().trim();
    if (!id || reserved.has(id) || [...ids.values()].includes(id))
      throw new Error('Copied Cube node identity is unavailable.');
    ids.set(String(node.id), id);
    node.id = id;
  }
  for (const link of Array.isArray(data.links) ? data.links : []) {
    if (Array.isArray(link)) {
      link[1] = ids.get(String(link[1])) ?? link[1];
      link[3] = ids.get(String(link[3])) ?? link[3];
    } else if (isRecord(link)) {
      link.origin_id = ids.get(String(link.origin_id)) ?? link.origin_id;
      link.target_id = ids.get(String(link.target_id)) ?? link.target_id;
    }
  }
  remapNativeReferences(data.widgets, ids);
  for (const node of data.nodes)
    if (isRecord(node) && isRecord(node.properties))
      remapNativeReferences(node.properties.previewExposures, ids);
  return data;
}

/** Rewrite native promoted and preview references without interpreting user widget values. */
function remapNativeReferences(value: unknown, ids: ReadonlyMap<string, string>): void {
  if (Array.isArray(value)) {
    for (const item of value) remapNativeReferences(item, ids);
    return;
  }
  if (!isRecord(value)) return;
  for (const [key, item] of Object.entries(value)) {
    if (key === 'nodeId') value[key] = ids.get(String(item)) ?? item;
    else if (isRecord(item) || Array.isArray(item)) remapNativeReferences(item, ids);
  }
}
