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
/** Persist validated portable Cube content beside its native Comfy definition. */

import type { NativeCubeSubgraph } from '../cube/ComfyCubeGraphBuilder.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

export interface CubeDocumentIdentity {
  cubeId: string;
  cubeVersion: string;
}

/** Validate and copy one canonical Cube document into graph-owned persistence. */
export function writeCubeDefinitionDocument(
  subgraph: NativeCubeSubgraph,
  document: UnknownRecord | undefined,
  identity: CubeDocumentIdentity,
): void {
  if (document === undefined) return;
  requireDocumentShape(document);
  const cubeId = requireString(document.cube_id, 'document.cube_id');
  const version = requireString(document.version, 'document.version');
  if (cubeId !== identity.cubeId || version !== identity.cubeVersion) {
    throw new TypeError('Cube document identity disagrees with its native definition.');
  }
  subgraph.extra = {
    ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
    sugarcubes_document: cloneRecord(document),
  };
}

/** Require the portable sections needed for backend canonical validation. */
function requireDocumentShape(document: UnknownRecord): void {
  for (const section of ['implementation', 'surface', 'flavors'] as const) {
    if (!isRecord(document[section])) {
      throw new TypeError(`Cube ${section} document section must be an object.`);
    }
  }
}

/** Require one non-empty document identity value. */
function requireString(value: unknown, path: string): string {
  const normalized = typeof value === 'string' ? value.trim() : '';
  if (!normalized) throw new TypeError(`Cube ${path} must be a non-empty string.`);
  return normalized;
}

/** Detach JSON-domain content from the import response. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}
