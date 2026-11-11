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
/** Persist renderer-neutral authored geometry on native Cube definitions. */

import {
  readAuthoredLayoutBaseline,
  readPayloadAuthoredLayoutBaseline,
  type AuthoredLayoutBaseline,
} from '../../geometry/AuthoredLayoutBaseline.js';
import type { ImportPayload } from '../../import/PlacementPayload.js';
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';

export type { AuthoredLayoutBaseline } from '../../geometry/AuthoredLayoutBaseline.js';

/** Name the definition-owned runtime baseline used for native editor stabilization. */
export const NATIVE_CUBE_AUTHORED_LAYOUT_KEY = 'sugarcubes_authored_layout';

interface NativeCubeLayoutOwner {
  extra?: UnknownRecord;
}

/** Attach a graph-local baseline without retaining the source workflow translation. */
export function attachNativeCubeAuthoredLayout(
  subgraph: NativeCubeLayoutOwner,
  payload: ImportPayload,
): AuthoredLayoutBaseline | null {
  const source = readPayloadAuthoredLayoutBaseline(payload);
  if (!source) return null;
  const baseline: AuthoredLayoutBaseline = {
    ...source,
    origin: [0, 0],
    group: null,
  };
  subgraph.extra = {
    ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
    [NATIVE_CUBE_AUTHORED_LAYOUT_KEY]: baseline,
  };
  return baseline;
}

/** Read the validated renderer-neutral baseline from one native Cube definition. */
export function readNativeCubeAuthoredLayout(
  subgraph: NativeCubeLayoutOwner,
): AuthoredLayoutBaseline | null {
  if (!isRecord(subgraph.extra)) return null;
  return readAuthoredLayoutBaseline(subgraph.extra[NATIVE_CUBE_AUTHORED_LAYOUT_KEY]);
}
