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
/** Orchestrate creation of one first-class Cube from the native graph selection. */

import type {
  AuthoredCube,
  AuthoredCubeDraft,
  AuthoredCubeIdentity,
  CubeDraftIdentity,
} from '../cube/ComfyCubeAuthoringAdapter.js';
import type {
  CubeAuthoringDialogOptions,
  CubeAuthoringMetadataDraft,
  CubeAuthoringValues,
} from './CubeAuthoringDialog.js';
import { suggestCubeAuthoringIdentity } from './CubeAuthoringIdentity.js';
import type { CubeSaveDestination } from './CubeAuthoringIdentity.js';
import type { WritableCubePack } from '../packs/CubePackService.js';
import type { UnknownRecord } from '../types/common.js';
import type { CubeSaveOutcome } from '../save/CubeSaveService.js';

export interface CubeCreationAuthoring {
  selectedCount(): number;
  validateSelection(): void;
  createEmptyDraft(identity: CubeDraftIdentity): AuthoredCubeDraft;
  createDraftFromSelection(identity: CubeDraftIdentity): AuthoredCubeDraft;
  validateSelectedSubgraph(): void;
  createDraftFromSelectedSubgraph(identity: CubeDraftIdentity): AuthoredCubeDraft;
  promoteDraft(instanceId: string, identity: AuthoredCubeIdentity): AuthoredCube;
  restoreDraft(instanceId: string): void;
}

interface CreationBrowser {
  getCubes?(): UnknownRecord[];
  getModelSuggestions?(): readonly string[];
}

interface CreationDialogs {
  openCubeAuthoring?(options: CubeAuthoringDialogOptions): Promise<CubeAuthoringValues | null>;
}

interface CreationToast {
  push?(severity: string, summary: string, detail?: string): unknown;
}

interface CreationLogger {
  error?(message: string, error?: unknown): void;
}

interface CreationSave {
  save(options: { cubeIds: string[] }): Promise<CubeSaveOutcome>;
}

interface CreationPackService {
  chooseWritablePack(): Promise<WritableCubePack | null>;
}

interface CreationOperation {
  validate(authoring: CubeCreationAuthoring): void;
  create(authoring: CubeCreationAuthoring, identity: CubeDraftIdentity): AuthoredCubeDraft;
}

export interface CubeCreationDependencies {
  getAuthoring(): CubeCreationAuthoring;
  cubeSave: CreationSave;
  cubeBrowser?: CreationBrowser | null;
  dialogs?: CreationDialogs | null;
  toast?: CreationToast | null;
  logger?: CreationLogger | null;
  packService?: CreationPackService | null;
  createInstanceId?: () => string;
}

/** Describe the metadata a Cube-editor HUD needs before its first persistence. */
/** Coordinate identity confirmation, native conversion, and initial persistence. */
export class CubeCreationService {
  readonly #getAuthoring: () => CubeCreationAuthoring;
  readonly #cubeSave: CreationSave;
  readonly #cubeBrowser: CreationBrowser | null;
  readonly #dialogs: CreationDialogs | null;
  readonly #toast: CreationToast | null;
  readonly #logger: CreationLogger | null;
  readonly #packService: CreationPackService | null;
  readonly #createInstanceId: () => string;

  /** Bind focused application collaborators without importing host globals. */
  constructor(options: CubeCreationDependencies) {
    this.#getAuthoring = options.getAuthoring;
    this.#cubeSave = options.cubeSave;
    this.#cubeBrowser = options.cubeBrowser ?? null;
    this.#dialogs = options.dialogs ?? null;
    this.#toast = options.toast ?? null;
    this.#logger = options.logger ?? null;
    this.#packService = options.packService ?? null;
    this.#createInstanceId = options.createInstanceId ?? createUuid;
  }

  /** Create a workflow-only draft from Comfy's authoritative selection conversion. */
  async startCreateCubeFromSelection(): Promise<AuthoredCubeDraft | null> {
    return this.#startCreation({
      validate: (authoring) => authoring.validateSelection(),
      create: (authoring, identity) => authoring.createDraftFromSelection(identity),
    });
  }

  /** Mark an existing native subgraph as a workflow-only Cube draft. */
  async startCreateCubeFromSelectedSubgraph(): Promise<AuthoredCubeDraft | null> {
    return this.#startCreation({
      validate: (authoring) => authoring.validateSelectedSubgraph(),
      create: (authoring, identity) => authoring.createDraftFromSelectedSubgraph(identity),
    });
  }

  /** Create an empty workflow-only Cube draft ready for native subgraph editing. */
  async startCreateEmptyCube(): Promise<AuthoredCubeDraft | null> {
    return this.#startCreation({
      validate: () => undefined,
      create: (authoring, identity) => authoring.createEmptyDraft(identity),
    });
  }

  /** Collect first-save metadata, promote a draft in place, and use the established save contract. */
  async saveDraft(instanceId: string): Promise<AuthoredCube | null> {
    try {
      const existingCubeIds = this.#existingCubeIds();
      const values = await this.#dialogs?.openCubeAuthoring?.({
        candidate: { defaultAlias: 'SugarCube', targetModel: 'SDXL', warnings: [] },
        modelSuggestions: this.#cubeBrowser?.getModelSuggestions?.() ?? [],
        deriveIdentity: async (name, targetModel, destination) =>
          this.#deriveIdentity(name, targetModel, destination, existingCubeIds),
      });
      if (!values) return null;
      return await this.#persistDraft(instanceId, values);
    } catch (error: unknown) {
      this.#reportDraftSaveFailure(error);
      return null;
    }
  }

  /** Save a draft directly from the Cube-editor metadata HUD. */
  async saveDraftFromEditor(
    instanceId: string,
    request: CubeAuthoringMetadataDraft,
  ): Promise<AuthoredCube | null> {
    try {
      const values = await this.#dialogs?.openCubeAuthoring?.({
        candidate: {
          defaultAlias: request.defaultAlias,
          description: request.description,
          destination: request.destination,
          supportedModels: request.supportedModels,
          targetModel: request.targetModel,
          warnings: [],
        },
        modelSuggestions: this.#cubeBrowser?.getModelSuggestions?.() ?? [],
        deriveIdentity: async (name, targetModel, destination) =>
          this.#deriveIdentity(name, targetModel, destination, this.#existingCubeIds()),
      });
      if (!values) return null;
      return await this.#persistDraft(instanceId, values);
    } catch (error: unknown) {
      this.#reportDraftSaveFailure(error);
      throw error;
    }
  }

  /** Perform one native authoring operation without assigning a persistent Cube identity. */
  async #startCreation(operation: CreationOperation): Promise<AuthoredCubeDraft | null> {
    try {
      const authoring = this.#getAuthoring();
      operation.validate(authoring);
      const authored = operation.create(authoring, {
        defaultAlias: 'Untitled Cube',
        instanceId: this.#createInstanceId(),
      });
      this.#toast?.push?.(
        'info',
        'Cube draft created',
        'Wire its native subgraph, then choose Save Cube from the Cube actions menu.',
      );
      return authored;
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : 'Unable to create SugarCube.';
      this.#toast?.push?.('error', 'SugarCube creation failed', message);
      this.#logger?.error?.('SugarCubes: native Cube creation failed', error);
      return null;
    }
  }

  /** Promote one graph-only draft and route it through the established Cube save contract. */
  async #persistDraft(
    instanceId: string,
    values: CubeAuthoringValues,
  ): Promise<AuthoredCube | null> {
    const authoring = this.#getAuthoring();
    let promotionStarted = false;
    try {
      const authored = authoring.promoteDraft(instanceId, {
        cubeId: values.cubeId,
        defaultAlias: values.defaultAlias,
        instanceId,
        targetModel: values.targetModel,
        supportedModels: values.supportedModels,
        description: values.description,
      });
      promotionStarted = true;
      const saveOutcome = await this.#cubeSave.save({ cubeIds: [values.cubeId] });
      if (saveOutcome.status === 'saved' && saveOutcome.savedCubeIds.includes(values.cubeId)) {
        this.#toast?.push?.(
          'success',
          'SugarCube saved',
          `${values.defaultAlias} is ready to reuse.`,
        );
        return authored;
      }
      authoring.restoreDraft(instanceId);
      throw new Error(
        saveOutcome.message ||
          'The Cube remains in the graph. Use Save Cube after resolving the issue.',
      );
    } catch (error: unknown) {
      if (promotionStarted) authoring.restoreDraft(instanceId);
      throw error;
    }
  }

  /** Report a first-save failure through the same application feedback owner. */
  #reportDraftSaveFailure(error: unknown): void {
    const message = error instanceof Error ? error.message : 'Unable to save SugarCube.';
    this.#toast?.push?.('error', 'SugarCube save failed', message);
    this.#logger?.error?.('SugarCubes: first Cube save failed', error);
  }

  /** Return the library identities reserved before a first-save destination is chosen. */
  #existingCubeIds(): string[] {
    return (this.#cubeBrowser?.getCubes?.() ?? [])
      .map((entry) => (typeof entry.cube_id === 'string' ? entry.cube_id : ''))
      .filter(Boolean);
  }

  /** Resolve a local or selected author-pack destination into the established canonical id. */
  async #deriveIdentity(
    name: string,
    targetModel: string,
    destination: CubeSaveDestination,
    existingCubeIds: readonly string[],
  ) {
    let resolvedDestination = destination;
    if (destination.kind === 'pack' && (!destination.owner || !destination.repo)) {
      const pack = await this.#packService?.chooseWritablePack();
      if (!pack) throw new Error('Choose a writable author-owned Cube Pack to continue.');
      resolvedDestination = { kind: 'pack', ...pack };
    }
    return suggestCubeAuthoringIdentity(name, targetModel, resolvedDestination, existingCubeIds);
  }
}

/** Create a stable instance id without importing a host-private UUID helper. */
function createUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  return `sugarcube-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
