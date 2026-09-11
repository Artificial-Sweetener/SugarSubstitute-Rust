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
/** Write finalized portable documents to graph-owned Cube definitions. */

import type { CubeDocumentIdentity } from '../../workflow/CubeDefinitionDocumentWriter.js';
import { writeCubeDefinitionDocument } from '../../workflow/CubeDefinitionDocumentWriter.js';
import type { UnknownRecord } from '../../types/common.js';
import type { CubeNodeCatalog } from './CubeNodeCatalog.js';

/** Update exactly the native Cube definitions addressed by stable instance id. */
export function writeCubeNodeDocumentsForIds(
  catalog: CubeNodeCatalog,
  instanceIds: readonly string[],
  document: UnknownRecord,
  identity: CubeDocumentIdentity,
): number {
  const targets = new Set(instanceIds.map((value) => value.trim()).filter(Boolean));
  let updated = 0;
  for (const instanceId of targets) {
    const node = catalog.get(instanceId);
    if (!node) continue;
    writeCubeDefinitionDocument(node.subgraph, document, identity);
    catalog.changed(node);
    updated += 1;
  }
  return updated;
}
