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
/** Filter and rank Cubes that can follow one live Cube through proximity. */
import { normalizeTargetModel } from '../core/ModelTargets.js';
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
/** Own boundary compatibility, model grouping, and deterministic search ranking. */
export class CubeAddCandidateCatalog {
    #compatibility;
    #strict;
    /** Bind Comfy's authoritative connection policy and live strictness setting. */
    constructor(options) {
        this.#compatibility = options.compatibility;
        this.#strict = options.strict;
    }
    /** Return every proximity-compatible catalog entry grouped by model compatibility. */
    candidates(source, entries) {
        const outputTypes = [
            ...new Set(source.outputs.filter((output) => !hasLiveLink(output)).map((output) => output.type)),
        ];
        const sourceModel = readSourceModel(source);
        const strict = this.#strict();
        const compatibility = createCompatibilityLookup(this.#compatibility, outputTypes, strict);
        const candidates = entries
            .filter(({ descriptor }) => descriptor.inputs.some((input) => compatibility.acceptsInput(input.type)))
            .map(({ type, descriptor }) => {
            const sameModel = modelsShareTargetFamily(sourceModel, descriptor.targetModel);
            return {
                type,
                cubeId: descriptor.cubeId,
                displayName: descriptor.displayName,
                description: descriptor.description,
                targetModel: descriptor.targetModel,
                sameModel,
                searchText: [descriptor.displayName, descriptor.description, ...descriptor.searchTerms]
                    .join(' ')
                    .toLocaleLowerCase(),
            };
        })
            .sort(compareCandidates);
        return {
            sameModel: candidates.filter((candidate) => candidate.sameModel),
            otherModels: candidates.filter((candidate) => !candidate.sameModel),
        };
    }
    /** Search every eligible Cube while preserving same-model priority. */
    search(groups, query) {
        const normalized = query.trim().toLocaleLowerCase();
        const all = [...groups.sameModel, ...groups.otherModels];
        return normalized ? all.filter((candidate) => candidate.searchText.includes(normalized)) : all;
    }
}
/** Cache repeated Comfy compatibility decisions for one synchronous menu-open scan. */
function createCompatibilityLookup(compatibility, outputTypes, strict) {
    const decisions = new Map();
    return {
        acceptsInput(inputType) {
            const cached = decisions.get(inputType);
            if (cached !== undefined)
                return cached;
            const accepted = outputTypes.some((outputType) => compatibility.accepts(outputType, inputType, strict));
            decisions.set(inputType, accepted);
            return accepted;
        },
    };
}
/** Read the current Cube's authoritative target family. */
function readSourceModel(source) {
    const identity = requireCubeIdentity(source);
    return normalizeTargetModel(identity.target_model);
}
/** Match only authoritative target families while retaining explicit Any neutrality. */
function modelsShareTargetFamily(sourceModel, candidateModel) {
    const source = normalizeTargetModel(sourceModel).toLocaleLowerCase();
    const candidate = normalizeTargetModel(candidateModel).toLocaleLowerCase();
    if (!source || !candidate)
        return false;
    return source === 'any' || candidate === 'any' || source === candidate;
}
/** Exclude outputs already owned by a persisted Comfy link from proximity discovery. */
function hasLiveLink(output) {
    const links = Array.isArray(output.links) ? output.links : [];
    return links.some((link) => link != null);
}
/** Keep exact-model results ahead of other models and sort labels stably. */
function compareCandidates(left, right) {
    return (Number(right.sameModel) - Number(left.sameModel) ||
        left.displayName.localeCompare(right.displayName, undefined, { sensitivity: 'base' }) ||
        left.cubeId.localeCompare(right.cubeId));
}
