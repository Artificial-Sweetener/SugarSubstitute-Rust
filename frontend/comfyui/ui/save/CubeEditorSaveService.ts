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
/** Coordinate modal-confirmed saves initiated from the native Cube editor. */

import type { CubeAuthoringMetadataDraft } from '../create/CubeAuthoringDialog.js';
import type { CubeAuthoringService } from '../create/CubeAuthoringService.js';
import {
  isDraftCubeNode,
  requireCubeIdentity,
  type CubeNode,
} from '../cube/node/ComfyCubeNodeFactory.js';
import { updateCubeNodeIdentityForIds } from '../cube/node/CubeNodeIdentityWriter.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import {
  readCubeNodeAuthoringCandidate,
  type CubeNodeAuthoringCandidate,
} from '../cube/node/CubeNodeAuthoringCandidate.js';
import type { CubeSaveOutcome } from './CubeSaveService.js';

export type CubeEditorSaveOutcome = 'saved' | 'cancelled';

interface DraftSave {
  saveDraftFromEditor(
    instanceId: string,
    request: CubeAuthoringMetadataDraft,
    candidateDetails: CubeNodeAuthoringCandidate,
  ): Promise<unknown | null>;
}

interface ExistingCubeSave {
  save(options: { cubeIds: string[] }): Promise<CubeSaveOutcome>;
}

export interface CubeEditorSaveServiceOptions {
  getCatalog(): CubeNodeCatalog | null;
  authoring: Pick<CubeAuthoringService, 'openExistingSave'>;
  cubeCreation: DraftSave;
  cubeSave: ExistingCubeSave;
}

/** Preserve the original authoring modal while saving exactly one edited Cube. */
export class CubeEditorSaveService {
  readonly #getCatalog: () => CubeNodeCatalog | null;
  readonly #authoring: Pick<CubeAuthoringService, 'openExistingSave'>;
  readonly #cubeCreation: DraftSave;
  readonly #cubeSave: ExistingCubeSave;

  constructor(options: CubeEditorSaveServiceOptions) {
    this.#getCatalog = options.getCatalog;
    this.#authoring = options.authoring;
    this.#cubeCreation = options.cubeCreation;
    this.#cubeSave = options.cubeSave;
  }

  /** Confirm metadata in the full modal before invoking the established save owner. */
  async save(
    node: CubeNode,
    metadataDraft: CubeAuthoringMetadataDraft,
  ): Promise<CubeEditorSaveOutcome> {
    const identity = requireCubeIdentity(node);
    const instanceId = readRequiredIdentity(identity.instance_id, 'instance');
    const candidateDetails = readCubeNodeAuthoringCandidate(node);
    if (isDraftCubeNode(node)) {
      const saved = await this.#cubeCreation.saveDraftFromEditor(
        instanceId,
        metadataDraft,
        candidateDetails,
      );
      return saved ? 'saved' : 'cancelled';
    }

    const cubeId = readRequiredIdentity(identity.cube_id, 'saved');
    const values = await this.#authoring.openExistingSave({
      ...candidateDetails,
      cubeId,
      defaultAlias: metadataDraft.defaultAlias,
      description: metadataDraft.description,
      destination: metadataDraft.destination,
      supportedModels: metadataDraft.supportedModels,
      targetModel: metadataDraft.targetModel,
      warnings: [],
    });
    if (!values) return 'cancelled';

    const catalog = this.#getCatalog();
    if (!catalog) throw new Error('SugarCubes runtime is unavailable.');
    const updated = updateCubeNodeIdentityForIds(catalog, [instanceId], {
      defaultAlias: values.defaultAlias,
      targetModel: values.targetModel,
      supportedModels: values.supportedModels,
      description: values.description,
    });
    if (!updated) throw new Error('The Cube is no longer available to save.');
    const outcome = await this.#cubeSave.save({ cubeIds: [cubeId] });
    if (outcome.status !== 'saved' || !outcome.savedCubeIds.includes(cubeId)) {
      throw new Error(outcome.message || 'The Cube was not saved.');
    }
    return 'saved';
  }
}

/** Require one graph-owned identity field at the save boundary. */
function readRequiredIdentity(value: unknown, kind: 'instance' | 'saved'): string {
  const normalized = typeof value === 'string' ? value.trim() : '';
  if (normalized) return normalized;
  throw new Error(
    kind === 'instance'
      ? 'The Cube is missing its instance identity.'
      : 'The Cube is missing its saved identity.',
  );
}
