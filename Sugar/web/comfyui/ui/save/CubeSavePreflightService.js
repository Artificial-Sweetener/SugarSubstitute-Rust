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
/** Gate the existing save workflow before identity, graph, or persistence mutation begins. */
/** Preserve the established save surface while one focused owner enforces preconditions. */
export class CubeSavePreflightService {
    #workflow;
    #validate;
    #feedback;
    /** Bind save orchestration behind a mutation-free validation boundary. */
    constructor(options) {
        this.#workflow = options.workflow;
        this.#validate = options.validate;
        this.#feedback = options.feedback ?? null;
    }
    /** Validate graph integrity before the normal public save operation. */
    async save(request = {}) {
        const failure = this.#preflight();
        return failure ?? this.#workflow.save(request);
    }
    /** Validate graph integrity before callers using the explicit implementation operation. */
    async saveImplementation(request = {}) {
        const failure = this.#preflight();
        return failure ?? this.#workflow.saveImplementation(request);
    }
    /** Convert a validation failure into the established non-throwing save result. */
    #preflight() {
        try {
            this.#validate();
            return null;
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.#feedback?.push?.('error', 'SugarCube save blocked', message);
            return { status: 'failed', savedCubeIds: [], message };
        }
    }
}
