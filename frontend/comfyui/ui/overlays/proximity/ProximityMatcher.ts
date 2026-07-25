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
/** Match compatible Cube boundary endpoints without owning host discovery. */

import type { ComfyGraph } from '../../types/graph.js';
import {
  CubeConnectionTypePolicy,
  normalizeCubePortType,
  type CubeConnectionLiteGraph,
} from '../../cube/connection/CubeConnectionTypePolicy.js';
import type {
  ProximityEndpointSource,
  ProximityInputEndpoint,
  ProximityMatch,
  ProximityOutputEndpoint,
} from './ProximityModel.js';

export interface ProximityMatchSettings {
  radius?: number;
  strict?: boolean;
}

export type ProximityLiteGraph = CubeConnectionLiteGraph;

/** Own deterministic distance, alias, type, and one-to-one matching policy. */
export class ProximityMatcher {
  readonly #endpoints: ProximityEndpointSource;
  readonly #compatibility: CubeConnectionTypePolicy;

  /** Bind endpoint discovery and Comfy's authoritative type-compatibility API. */
  constructor(
    endpoints: ProximityEndpointSource,
    getLiteGraph: () => ProximityLiteGraph | null,
    logger: Pick<Console, 'debug'>,
  ) {
    this.#endpoints = endpoints;
    this.#compatibility = new CubeConnectionTypePolicy({ getLiteGraph, logger });
  }

  /** Compute stable nearest compatible matches for all available boundary slots. */
  compute(
    graph: ComfyGraph | null | undefined,
    settings: ProximityMatchSettings,
  ): ProximityMatch[] {
    if (!graph || !Array.isArray(graph._nodes)) return [];
    const radius = Number(settings.radius) || 160;
    const radiusSquared = radius * radius;
    const strict = Boolean(settings.strict);
    const endpoints = this.#endpoints.discover(graph);
    const candidates: Candidate[] = [];
    for (const output of endpoints.outputs) {
      for (const input of endpoints.inputs) {
        if (isSameInstance(output, input)) continue;
        if (!output.cube && !input.cube) continue;
        const dx = output.slotPos[0] - input.slotPos[0];
        const dy = output.slotPos[1] - input.slotPos[1];
        const distanceSquared = dx * dx + dy * dy;
        if (distanceSquared > radiusSquared) continue;
        if (!this.#compatibility.accepts(output.type, input.type, strict)) continue;
        const aliasMatch = Boolean(output.alias && input.alias && output.alias === input.alias);
        const typeMatch = normalizeCubePortType(output.type) === normalizeCubePortType(input.type);
        candidates.push({
          output,
          input,
          distanceSquared,
          score: Math.sqrt(distanceSquared) - (aliasMatch ? 40 : 0) - (typeMatch ? 20 : 0),
        });
      }
    }
    candidates.sort((left, right) => left.score - right.score);
    const usedOutputs = new Set<string>();
    const usedInputs = new Set<string>();
    const matches: ProximityMatch[] = [];
    for (const candidate of candidates) {
      if (usedOutputs.has(candidate.output.key) || usedInputs.has(candidate.input.key)) {
        continue;
      }
      usedOutputs.add(candidate.output.key);
      usedInputs.add(candidate.input.key);
      matches.push(toMatch(candidate));
    }
    return matches;
  }
}

interface Candidate {
  output: ProximityOutputEndpoint;
  input: ProximityInputEndpoint;
  distanceSquared: number;
  score: number;
}

/** Convert one selected candidate into overlay and prompt-routing data. */
function toMatch(candidate: Candidate): ProximityMatch {
  const { output, input } = candidate;
  return {
    outputId: output.endpointId,
    outputCube: output.cube,
    outputSlot: output.slot,
    ...(output.node ? { outputNode: output.node } : {}),
    outputPos: output.slotPos,
    outputType: output.type,
    inputId: input.endpointId,
    inputCube: input.cube,
    inputSlot: input.slot,
    inputName: input.slotName,
    ...(input.node ? { inputNode: input.node } : {}),
    inputPos: input.slotPos,
    inputType: input.type,
    originId: output.originId,
    originSlot: output.originSlot,
    promptTargets: input.promptTargets,
    distance: Math.sqrt(candidate.distanceSquared),
    candidateDetails: {
      outType: output.type,
      inType: input.type,
      aliasMatch: Boolean(output.alias && output.alias === input.alias),
      typeMatch: normalizeCubePortType(output.type) === normalizeCubePortType(input.type),
    },
  };
}

/** Exclude a Cube from auto-connecting to another boundary on itself. */
function isSameInstance(output: ProximityOutputEndpoint, input: ProximityInputEndpoint): boolean {
  if (output.instanceId && input.instanceId) {
    return String(output.instanceId) === String(input.instanceId);
  }
  return Boolean(output.cube && input.cube && output.cube === input.cube);
}
