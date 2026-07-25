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
import { normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { suggestPersonalCubeIdentity } from './PersonalCubeIdentity.js';
/** Coordinate identity confirmation, native conversion, and initial persistence. */
export class CubeCreationService {
    #getAuthoring;
    #cubeSave;
    #cubeBrowser;
    #dialogs;
    #toast;
    #logger;
    #createInstanceId;
    /** Bind focused application collaborators without importing host globals. */
    constructor(options) {
        this.#getAuthoring = options.getAuthoring;
        this.#cubeSave = options.cubeSave;
        this.#cubeBrowser = options.cubeBrowser ?? null;
        this.#dialogs = options.dialogs ?? null;
        this.#toast = options.toast ?? null;
        this.#logger = options.logger ?? null;
        this.#createInstanceId = options.createInstanceId ?? createUuid;
    }
    /** Create and save one Cube using Comfy's authoritative native conversion. */
    async startCreateCubeFromSelection() {
        try {
            const authoring = this.#getAuthoring();
            authoring.validateSelection();
            const selectedCount = authoring.selectedCount();
            const existingCubeIds = (this.#cubeBrowser?.getCubes?.() ?? [])
                .map((entry) => (typeof entry.cube_id === 'string' ? entry.cube_id : ''))
                .filter(Boolean);
            const identity = await this.#dialogs?.openCreatePersonalCube?.({
                candidate: {
                    defaultAlias: 'SugarCube',
                    nodeIds: Array.from({ length: selectedCount }, (_, index) => index),
                    warnings: [],
                },
                deriveIdentity: (name) => suggestPersonalCubeIdentity(name, existingCubeIds),
            });
            if (!identity)
                return null;
            const defaultAlias = normalizeDefaultAliasTitle(identity.defaultAlias);
            const authored = authoring.createFromSelection({
                cubeId: identity.cubeId,
                defaultAlias,
                instanceId: this.#createInstanceId(),
                targetModel: '',
                supportedModels: [],
                description: '',
            });
            await this.#cubeSave.save({ cubeIds: [identity.cubeId] });
            this.#toast?.push?.('success', 'SugarCube created', `${defaultAlias} is now a native Cube.`);
            return authored;
        }
        catch (error) {
            const message = error instanceof Error ? error.message : 'Unable to create SugarCube.';
            this.#toast?.push?.('error', 'SugarCube creation failed', message);
            this.#logger?.error?.('SugarCubes: native Cube creation failed', error);
            return null;
        }
    }
}
/** Create a stable instance id without importing a host-private UUID helper. */
function createUuid() {
    if (typeof globalThis.crypto?.randomUUID === 'function') {
        return globalThis.crypto.randomUUID();
    }
    return `sugarcube-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
