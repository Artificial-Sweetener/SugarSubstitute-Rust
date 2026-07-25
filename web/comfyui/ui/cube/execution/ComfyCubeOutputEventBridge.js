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
/** Capture Cube marker results before Comfy drops unresolvable execution IDs. */
import { isRecord } from '../../types/common.js';
import { isCubeOutputExecutionId } from './CubeOutputExecutionIdentity.js';
/**
 * Own the Comfy executed-event boundary for prompt-only Cube output sinks.
 *
 * Comfy deliberately refuses to write an execution result into its node-output
 * map when the execution ID has no corresponding graph node. Loaded CubeOutput
 * markers are prompt-only, so SugarCubes retains their results at this adapter.
 */
export class ComfyCubeOutputEventBridge {
    #api;
    #store;
    #logger;
    #installed = false;
    /** Bind Comfy's event API to the Cube-owned execution result store. */
    constructor(options) {
        this.#api = options.api;
        this.#store = options.store;
        this.#logger = options.logger ?? null;
    }
    /** Install one stable executed-event listener. */
    install() {
        if (this.#installed)
            return;
        this.#api.addEventListener('executed', this.#handleExecuted);
        this.#installed = true;
    }
    /** Remove only this bridge's listener. */
    dispose() {
        if (!this.#installed)
            return;
        this.#api.removeEventListener('executed', this.#handleExecuted);
        this.#installed = false;
    }
    /** Capture the native marker result under its prompt-only execution ID. */
    #handleExecuted = (event) => {
        if (!isRecord(event) || !isRecord(event.detail))
            return;
        const detail = event.detail;
        const executionId = [detail.node, detail.display_node].find(isCubeOutputExecutionId);
        if (!executionId || !('output' in detail))
            return;
        this.#store.retain(executionId, detail.output);
        this.#logger?.debug('SugarCubes captured a loaded Cube output result.', {
            executionId,
        });
    };
}
