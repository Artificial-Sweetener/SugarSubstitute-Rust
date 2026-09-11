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
/** Adapt graph-owned native Cube nodes to the save application boundary. */

import { listCubeNodeInstances, type CubeNodeInstance } from './CubeNodeInstanceCatalog.js';
import {
  updateCubeNodeIdentityForIds,
  type CubeNodeIdentityUpdates,
} from './CubeNodeIdentityWriter.js';
import type { CubeNodeCatalog } from './CubeNodeCatalog.js';
import { writeCubeNodeDocumentsForIds } from './CubeNodeDocumentWriter.js';
import type { CubeDocumentIdentity } from '../../workflow/CubeDefinitionDocumentWriter.js';
import type { UnknownRecord } from '../../types/common.js';

interface ComfyCubeSaveAdapterDependencies {
  getCatalog: () => CubeNodeCatalog | null;
}

/** Isolate live Comfy node lookup and mutation from save orchestration. */
export class ComfyCubeSaveAdapter {
  readonly #getCatalog: () => CubeNodeCatalog | null;

  constructor({ getCatalog }: ComfyCubeSaveAdapterDependencies) {
    this.#getCatalog = getCatalog;
  }

  /** Snapshot every graph-owned Cube node into typed save input. */
  listInstances(): CubeNodeInstance[] {
    return listCubeNodeInstances(this.#getCatalog()?.list() ?? []);
  }

  /** Apply save identity changes to exactly the addressed Cube instances. */
  updateIdentities(instanceIds: readonly string[], updates: CubeNodeIdentityUpdates): number {
    const catalog = this.#getCatalog();
    return catalog ? updateCubeNodeIdentityForIds(catalog, instanceIds, updates) : 0;
  }

  /** Persist one finalized portable document beside each addressed native definition. */
  updateDocuments(
    instanceIds: readonly string[],
    document: UnknownRecord,
    identity: CubeDocumentIdentity,
  ): number {
    const catalog = this.#getCatalog();
    return catalog ? writeCubeNodeDocumentsForIds(catalog, instanceIds, document, identity) : 0;
  }
}
