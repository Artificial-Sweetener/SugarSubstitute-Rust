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
/** Inspect Comfy clipboard serialization before it can register copied definitions. */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';

/** Detect a copied wrapper or a wrapper buried inside a copied Subgraph definition. */
export function readCubeClipboardViolation(value: unknown, targetIsRoot: boolean): string | null {
  if (!isRecord(value)) return null;
  const definitions = Array.isArray(value.subgraphs) ? value.subgraphs : [];
  const cubeDefinitionIds = new Set<string>();
  for (const definition of definitions) {
    if (!isRecord(definition) || !isCubeKind(readExtraKind(definition))) continue;
    const id = readString(definition.id);
    if (id) cubeDefinitionIds.add(id);
  }
  const rootNodes = Array.isArray(value.nodes) ? value.nodes : [];
  if (!targetIsRoot && rootNodes.some((node) => isSerializedCubeNode(node, cubeDefinitionIds))) {
    return 'The copied SugarCube was not added and no copied definitions were registered.';
  }
  for (const definition of definitions) {
    if (!isRecord(definition) || !Array.isArray(definition.nodes)) continue;
    if (definition.nodes.some((node) => isSerializedCubeNode(node, cubeDefinitionIds))) {
      return 'The copied data contains a SugarCube nested inside a Subgraph and was not added.';
    }
  }
  return null;
}

/** Recognize serialized instance markers and pre-configuration definition identity. */
function isSerializedCubeNode(value: unknown, cubeDefinitionIds: ReadonlySet<string>): boolean {
  if (!isRecord(value)) return false;
  const properties = isRecord(value.properties) ? value.properties : null;
  return isCubeKind(properties?.sugarcubes_kind) || cubeDefinitionIds.has(readString(value.type));
}

/** Read definition-level product identity from untrusted clipboard data. */
function readExtraKind(value: UnknownRecord): unknown {
  return isRecord(value.extra) ? value.extra.sugarcubes_kind : null;
}

/** Recognize both persisted and workflow-only Cube identity markers. */
function isCubeKind(value: unknown): boolean {
  return value === 'cube' || value === 'cube_draft';
}

/** Read one stable serialized identifier. */
function readString(value: unknown): string {
  return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}
