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

import type { CubeConnectionCompatibility } from '../cube/connection/CubeConnectionTypePolicy.js';
import { normalizeTargetModel } from '../core/ModelTargets.js';
import { requireCubeIdentity, type CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubePickerCatalogEntry } from './CubePickerCatalogRegistry.js';

export interface CubeAddCandidate {
  type: string;
  cubeId: string;
  displayName: string;
  description: string;
  targetModel: string;
  sameModel: boolean;
  searchText: string;
}

export interface CubeAddCandidateGroups {
  sameModel: readonly CubeAddCandidate[];
  otherModels: readonly CubeAddCandidate[];
}

export interface CubeAddCandidateCatalogOptions {
  compatibility: CubeConnectionCompatibility;
  strict(): boolean;
}

/** Own boundary compatibility, model grouping, and deterministic search ranking. */
export class CubeAddCandidateCatalog {
  readonly #compatibility: CubeConnectionCompatibility;
  readonly #strict: () => boolean;

  /** Bind Comfy's authoritative connection policy and live strictness setting. */
  constructor(options: CubeAddCandidateCatalogOptions) {
    this.#compatibility = options.compatibility;
    this.#strict = options.strict;
  }

  /** Return every proximity-compatible catalog entry grouped by model compatibility. */
  candidates(source: CubeNode, entries: readonly CubePickerCatalogEntry[]): CubeAddCandidateGroups {
    const outputTypes = [
      ...new Set(
        source.outputs.filter((output) => !hasLiveLink(output)).map((output) => output.type),
      ),
    ];
    const sourceModel = readSourceModel(source);
    const strict = this.#strict();
    const compatibility = createCompatibilityLookup(this.#compatibility, outputTypes, strict);
    const candidates = entries
      .filter(({ descriptor }) =>
        descriptor.inputs.some((input) => compatibility.acceptsInput(input.type)),
      )
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
        } satisfies CubeAddCandidate;
      })
      .sort(compareCandidates);
    return {
      sameModel: candidates.filter((candidate) => candidate.sameModel),
      otherModels: candidates.filter((candidate) => !candidate.sameModel),
    };
  }

  /** Search every eligible Cube while preserving same-model priority. */
  search(groups: CubeAddCandidateGroups, query: string): readonly CubeAddCandidate[] {
    const normalized = query.trim().toLocaleLowerCase();
    const all = [...groups.sameModel, ...groups.otherModels];
    return normalized ? all.filter((candidate) => candidate.searchText.includes(normalized)) : all;
  }
}

interface CandidateCompatibilityLookup {
  acceptsInput(inputType: unknown): boolean;
}

/** Cache repeated Comfy compatibility decisions for one synchronous menu-open scan. */
function createCompatibilityLookup(
  compatibility: CubeConnectionCompatibility,
  outputTypes: readonly unknown[],
  strict: boolean,
): CandidateCompatibilityLookup {
  const decisions = new Map<unknown, boolean>();
  return {
    acceptsInput(inputType) {
      const cached = decisions.get(inputType);
      if (cached !== undefined) return cached;
      const accepted = outputTypes.some((outputType) =>
        compatibility.accepts(outputType, inputType, strict),
      );
      decisions.set(inputType, accepted);
      return accepted;
    },
  };
}

/** Read the current Cube's authoritative target family. */
function readSourceModel(source: CubeNode): string {
  const identity = requireCubeIdentity(source);
  return normalizeTargetModel(identity.target_model);
}

/** Match only authoritative target families while retaining explicit Any neutrality. */
function modelsShareTargetFamily(sourceModel: unknown, candidateModel: unknown): boolean {
  const source = normalizeTargetModel(sourceModel).toLocaleLowerCase();
  const candidate = normalizeTargetModel(candidateModel).toLocaleLowerCase();
  if (!source || !candidate) return false;
  return source === 'any' || candidate === 'any' || source === candidate;
}

/** Exclude outputs already owned by a persisted Comfy link from proximity discovery. */
function hasLiveLink(output: CubeNode['outputs'][number]): boolean {
  const links = Array.isArray(output.links) ? output.links : [];
  return links.some((link) => link != null);
}

/** Keep exact-model results ahead of other models and sort labels stably. */
function compareCandidates(left: CubeAddCandidate, right: CubeAddCandidate): number {
  return (
    Number(right.sameModel) - Number(left.sameModel) ||
    left.displayName.localeCompare(right.displayName, undefined, { sensitivity: 'base' }) ||
    left.cubeId.localeCompare(right.cubeId)
  );
}
