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
/** Coordinate selection-scoped version availability and switching for the toolbox. */
import { normalizeCubeVersion } from '../core/CubeDefinitionKey.js';
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
/** Own asynchronous state and stale-selection protection around the version presenter. */
export class CubeVersionToolboxController {
    #availability;
    #switcher;
    #presenter;
    #logger;
    #reportError;
    #anchor = null;
    #node = null;
    #options = [];
    #loading = false;
    #busy = false;
    #error = null;
    #generation = 0;
    /** Bind application services and presentation feedback. */
    constructor(options) {
        this.#availability = options.availability;
        this.#switcher = options.switcher;
        this.#presenter = options.presenter;
        this.#logger = options.logger;
        this.#reportError = options.reportError;
    }
    /** Present one selected persisted Cube after the supplied host action. */
    present(anchor, node) {
        const selectionChanged = this.#node !== node;
        if (selectionChanged) {
            this.#generation += 1;
            this.#node = node;
            this.#options = [];
            this.#loading = false;
            this.#busy = false;
            this.#error = null;
        }
        this.#anchor = anchor;
        const control = this.#render();
        if (selectionChanged)
            void this.#loadOptions();
        return control;
    }
    /** Clear selection-owned state and invalidate outstanding requests. */
    clear() {
        this.#generation += 1;
        this.#anchor = null;
        this.#node = null;
        this.#options = [];
        this.#loading = false;
        this.#busy = false;
        this.#error = null;
        this.#presenter.clear();
    }
    /** Release the focused presenter. */
    dispose() {
        this.clear();
        this.#presenter.dispose();
    }
    /** Build one immutable callback model for the current selected node. */
    #render() {
        if (!this.#anchor || !this.#node)
            throw new Error('Cube version selection is unavailable.');
        const currentVersion = readCurrentVersion(this.#node);
        const model = {
            currentVersion,
            options: this.#options,
            loading: this.#loading,
            busy: this.#busy,
            error: this.#error,
            select: (version) => void this.#select(version),
        };
        return this.#presenter.present(this.#anchor, model);
    }
    /** Load options once and ignore completions from an obsolete selection. */
    async #loadOptions() {
        if (!this.#node || this.#loading || this.#options.length > 0)
            return;
        const node = this.#node;
        const generation = this.#generation;
        const identity = requireCubeIdentity(node);
        const cubeId = readString(identity.cube_id);
        const currentVersion = readCurrentVersion(node);
        this.#loading = true;
        this.#error = null;
        this.#render();
        try {
            const options = await this.#availability.list(cubeId, currentVersion);
            if (generation !== this.#generation || node !== this.#node)
                return;
            this.#options = options;
        }
        catch (error) {
            if (generation !== this.#generation || node !== this.#node)
                return;
            this.#error = readError(error);
            this.#logger.error('SugarCubes: Cube version discovery failed.', error);
        }
        finally {
            if (generation === this.#generation && node === this.#node) {
                this.#loading = false;
                this.#render();
            }
        }
    }
    /** Execute one switch while preventing duplicate mutations and stale UI writes. */
    async #select(version) {
        if (!this.#node || this.#busy)
            return;
        const node = this.#node;
        const generation = this.#generation;
        this.#busy = true;
        this.#error = null;
        this.#render();
        try {
            const replacement = await this.#switcher.switch(node, version);
            if (generation !== this.#generation || node !== this.#node)
                return;
            this.#busy = false;
            this.#node = replacement;
            this.#render();
            return;
        }
        catch (error) {
            const message = readError(error);
            this.#logger.error('SugarCubes: Cube version switch failed.', error);
            this.#reportError(message);
        }
        finally {
            if (generation === this.#generation && node === this.#node && this.#busy) {
                this.#busy = false;
                this.#render();
            }
        }
    }
}
/** Read one normalized current version from persisted instance identity. */
function readCurrentVersion(node) {
    return normalizeCubeVersion(requireCubeIdentity(node).cube_version);
}
/** Read one trimmed identity string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Normalize unknown failures for SugarCubes feedback. */
function readError(error) {
    return error instanceof Error && error.message ? error.message : 'Cube version switch failed.';
}
