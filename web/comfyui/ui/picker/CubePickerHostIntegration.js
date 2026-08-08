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
/** Coordinate focused Cube picker adapters behind Comfy extension lifecycle hooks. */
/** Expose thin extension hooks while focused adapters retain policy ownership. */
export class CubePickerHostIntegration {
    #definitions;
    #creation;
    #results;
    #logger;
    #reportError;
    /** Bind current-version host adapters to one extension-facing coordinator. */
    constructor(options) {
        this.#definitions = options.definitions;
        this.#creation = options.creation;
        this.#results = options.results;
        this.#logger = options.logger;
        this.#reportError = options.reportError;
    }
    /** Contribute cached placement-ready definitions during initial registration. */
    async contribute(definitions) {
        try {
            await this.#definitions.contribute(definitions);
        }
        catch (error) {
            const detail = readErrorMessage(error);
            this.#logger.error('SugarCubes picker startup catalog failed.', { detail, error });
            this.#reportError('SugarCubes picker unavailable', detail);
        }
    }
    /** Restore Sugar definitions and compatibility ordering on each Vue refresh. */
    orderForVue(definitions) {
        this.#definitions.orderForVue(definitions);
        this.#results.schedule();
    }
    /** Activate the single shared native creation compatibility seam. */
    activate() {
        try {
            this.#results.install();
            this.#creation.install();
        }
        catch (error) {
            const detail = readErrorMessage(error);
            this.#logger.error('SugarCubes picker creation adapter failed to activate.', {
                detail,
                error,
            });
            this.#reportError('SugarCubes picker unavailable', detail);
        }
    }
    /** Reconcile pack changes without requiring a Comfy restart. */
    async refresh() {
        try {
            await this.#definitions.reconcile();
            this.#results.schedule();
        }
        catch (error) {
            const detail = readErrorMessage(error);
            this.#logger.error('SugarCubes picker catalog reconciliation failed.', { detail, error });
            this.#reportError('SugarCubes picker refresh failed', detail);
            throw error;
        }
    }
}
/** Normalize one caught lifecycle error for structured feedback. */
function readErrorMessage(error) {
    if (error instanceof Error && error.message.trim())
        return error.message.trim();
    return String(error || 'Unknown SugarCubes picker failure');
}
