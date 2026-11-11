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
/** Prepare Cube descriptions before Comfy registers serialized Subgraph node definitions. */

import { resolveCubeDefinitionDescription } from './node/CubeDefinitionIdentityWriter.js';
import { isRecord } from '../types/common.js';

const CUBE_KINDS = new Set(['cube', 'cube_draft']);

/** Adapt only marked serialized definitions during Comfy's official preconfigure hook. */
export class CubeSerializedDefinitionPresentationAdapter {
  /** Supply Cube metadata to generated native node definitions before registration. */
  prepare(workflow: unknown): number {
    if (!isRecord(workflow) || !isRecord(workflow.definitions)) return 0;
    const definitions = workflow.definitions.subgraphs;
    if (!Array.isArray(definitions)) return 0;
    let changed = 0;
    for (const definition of definitions) {
      if (!isRecord(definition) || !isRecord(definition.extra)) continue;
      if (!CUBE_KINDS.has(String(definition.extra.sugarcubes_kind))) continue;
      const metadata = definition.extra.sugarcubes_cube;
      if (!isRecord(metadata)) continue;
      definition.description = resolveCubeDefinitionDescription(metadata);
      changed += 1;
    }
    return changed;
  }
}
