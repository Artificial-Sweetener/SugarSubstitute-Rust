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
/** Stage version-specific embedded subgraphs without mutating live definitions. */
import { remapCubeEmbeddedSubgraphs } from './CubeEmbeddedSubgraphRemapper.js';
/** Own isolated embedded-definition registration and compensation. */
export class CubeVersionDefinitionStager {
    #registrar;
    #createId;
    /** Bind native registration and runtime identity generation. */
    constructor(options) {
        this.#registrar = options.registrar;
        this.#createId = options.createId;
    }
    /** Register an isolated clone or remove every staged definition on failure. */
    stage(payload) {
        const staged = remapCubeEmbeddedSubgraphs(payload, this.#createId);
        const registration = this.#registrar.register(staged.payload);
        if (registration.warnings.length === 0)
            return staged;
        this.#registrar.discard(staged.definitionIds);
        throw new Error(`Cube embedded definitions could not be staged: ${registration.warnings.join(' ')}`);
    }
    /** Remove staged definitions after construction or replacement compensation. */
    discard(staged) {
        this.#registrar.discard(staged.definitionIds);
    }
}
