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
/** Reconcile stale auxiliary Cube documents before native graph construction. */

import { isRecord } from '../types/common.js';

const CUBE_KINDS = new Set(['cube', 'cube_draft']);

/** Remove stale portable copies while preserving authoritative native definitions. */
export class CubeSerializedDocumentReconciler {
  /** Discard documents whose identity no longer matches their native definition. */
  prepare(workflow: unknown): number {
    if (!isRecord(workflow) || !isRecord(workflow.definitions)) return 0;
    const definitions = workflow.definitions.subgraphs;
    if (!Array.isArray(definitions)) return 0;

    let reconciled = 0;
    for (const definition of definitions) {
      if (!isRecord(definition) || !isRecord(definition.extra)) continue;
      const extra = definition.extra;
      if (!CUBE_KINDS.has(readString(extra.sugarcubes_kind))) continue;
      if (!isRecord(extra.sugarcubes_cube) || !isRecord(extra.sugarcubes_document)) continue;

      const nativeCubeId = readString(extra.sugarcubes_cube.cube_id);
      const nativeVersion = readString(extra.sugarcubes_cube.cube_version);
      const documentCubeId = readString(extra.sugarcubes_document.cube_id);
      const documentVersion = readString(extra.sugarcubes_document.version);
      if (
        nativeCubeId &&
        nativeVersion &&
        documentCubeId &&
        documentVersion &&
        (nativeCubeId !== documentCubeId || nativeVersion !== documentVersion)
      ) {
        Reflect.deleteProperty(extra, 'sugarcubes_document');
        reconciled += 1;
      }
    }
    return reconciled;
  }
}

/** Normalize one serialized identity value. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
