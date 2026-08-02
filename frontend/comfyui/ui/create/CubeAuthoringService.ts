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
/** Own the complete metadata-and-destination session used by every Cube save entry point. */

import { normalizeDefaultAliasTitle, parseCanonicalCubeId } from '../core/CubeId.js';
import type { UnknownRecord } from '../types/common.js';
import type {
  CubeAuthoringCandidate,
  CubeAuthoringDestinationOption,
  CubeAuthoringDialog,
  CubeAuthoringValues,
} from './CubeAuthoringDialog.js';
import { suggestCubeAuthoringIdentity } from './CubeAuthoringIdentity.js';
import type { CubeSaveDestination } from './CubeAuthoringIdentity.js';
import type { WritableCubePack } from '../packs/CubePackService.js';

interface AuthoringBrowser {
  getCubes?(): UnknownRecord[];
  getModelSuggestions?(): readonly string[];
}

interface AuthoringPackService {
  createAuthoringPackForClaimedOwner(): Promise<WritableCubePack | null>;
  listAuthoringPacks(): Promise<WritableCubePack[]>;
}

interface AuthoringToast {
  push?(severity: string, summary: string, detail?: string): unknown;
}

export interface CubeAuthoringServiceOptions {
  browser?: AuthoringBrowser | null;
  dialogs: CubeAuthoringDialog;
  packService: AuthoringPackService;
  toast?: AuthoringToast | null;
}

const LOCAL_DESTINATION: CubeAuthoringDestinationOption = {
  key: 'local/personal',
  label: 'Personal cubes',
  detail: 'Saved locally',
  destination: { kind: 'local' },
};

const CREATE_PACK_DESTINATION: CubeAuthoringDestinationOption = {
  key: 'create-pack',
  label: 'Create a new Cube Pack…',
  action: 'create-pack',
};

/** Coordinate the one canonical Cube authoring modal for first and subsequent saves. */
export class CubeAuthoringService {
  readonly #browser: AuthoringBrowser | null;
  readonly #dialogs: CubeAuthoringDialog;
  readonly #packService: AuthoringPackService;
  readonly #toast: AuthoringToast | null;

  constructor(options: CubeAuthoringServiceOptions) {
    this.#browser = options.browser ?? null;
    this.#dialogs = options.dialogs;
    this.#packService = options.packService;
    this.#toast = options.toast ?? null;
  }

  /** Open the complete first-save session with concrete writable destinations. */
  async openFirstSave(candidate: CubeAuthoringCandidate): Promise<CubeAuthoringValues | null> {
    const destinations = await this.#loadFirstSaveDestinations();
    return this.#dialogs.openCubeAuthoring({
      candidate,
      destinations,
      modelSuggestions: this.#browser?.getModelSuggestions?.() ?? [],
      deriveIdentity: (name, targetModel, destination) =>
        suggestCubeAuthoringIdentity(name, targetModel, destination, this.#existingCubeIds()),
      onCreateDestination: async () => {
        const pack = await this.#packService.createAuthoringPackForClaimedOwner();
        return pack ? packDestinationOption(pack) : null;
      },
    });
  }

  /** Open the same authoring session for a Cube whose storage identity cannot move. */
  async openExistingSave(candidate: CubeAuthoringCandidate): Promise<CubeAuthoringValues | null> {
    const cubeId = candidate.cubeId;
    if (!cubeId) throw new Error('The Cube is missing its saved identity.');
    const destination = destinationFromCubeId(cubeId);
    return this.#dialogs.openCubeAuthoring({
      candidate: { ...candidate, destination },
      destinationLocked: true,
      destinations: [destinationOption(destination)],
      modelSuggestions: this.#browser?.getModelSuggestions?.() ?? [],
      deriveIdentity: (name, targetModel) => {
        const normalizedName = normalizeDefaultAliasTitle(name);
        if (!normalizedName) throw new Error('Name is required.');
        return {
          name: normalizedName,
          defaultAlias: `${targetModel}/${normalizedName}`,
          cubeId,
        };
      },
    });
  }

  /** Load author-owned destinations without making local saving depend on pack availability. */
  async #loadFirstSaveDestinations(): Promise<CubeAuthoringDestinationOption[]> {
    try {
      const packs = await this.#packService.listAuthoringPacks();
      return [LOCAL_DESTINATION, ...packs.map(packDestinationOption), CREATE_PACK_DESTINATION];
    } catch (error: unknown) {
      this.#toast?.push?.(
        'warn',
        'Cube packs unavailable',
        error instanceof Error ? error.message : 'Only local saving is available right now.',
      );
      return [LOCAL_DESTINATION, CREATE_PACK_DESTINATION];
    }
  }

  /** Return canonical ids already reserved by the active library. */
  #existingCubeIds(): string[] {
    return (this.#browser?.getCubes?.() ?? [])
      .map((entry) => (typeof entry.cube_id === 'string' ? entry.cube_id : ''))
      .filter(Boolean);
  }
}

/** Present one concrete author-owned pack inside the save modal. */
function packDestinationOption(pack: WritableCubePack): CubeAuthoringDestinationOption {
  return {
    key: `pack/${pack.repoRef}`,
    label: pack.repo,
    detail: pack.owner,
    destination: { kind: 'pack', ...pack },
  };
}

/** Present the fixed storage destination of an existing Cube. */
function destinationOption(destination: CubeSaveDestination): CubeAuthoringDestinationOption {
  return destination.kind === 'local' ? LOCAL_DESTINATION : packDestinationOption(destination);
}

/** Recover the concrete save destination encoded by a canonical Cube id. */
function destinationFromCubeId(cubeId: string): CubeSaveDestination {
  const parsed = parseCanonicalCubeId(cubeId);
  return parsed.sourceKind === 'local'
    ? { kind: 'local' }
    : {
        kind: 'pack',
        owner: parsed.owner,
        repo: parsed.repo,
        repoRef: parsed.repoRef,
      };
}
