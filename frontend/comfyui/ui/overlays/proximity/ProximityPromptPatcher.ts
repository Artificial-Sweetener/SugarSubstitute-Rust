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
/** Apply transient Cube proximity links to Comfy's flattened execution prompt. */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import type { ProximityMatch } from './ProximityModel.js';

interface PromptNode extends UnknownRecord {
  inputs?: unknown[] | UnknownRecord;
}

interface PromptPayload extends UnknownRecord {
  output: Record<string, PromptNode>;
}

export interface ProximityPromptPatchResult {
  payload: unknown;
  applied: ProximityMatch[];
}

/** Own immutable prompt-only application of resolved proximity routes. */
export class ProximityPromptPatcher {
  /** Apply matches to flattened prompt targets without altering workflow serialization. */
  apply(payload: unknown, matches: readonly ProximityMatch[]): ProximityPromptPatchResult {
    if (!isPromptPayload(payload) || matches.length === 0) {
      return { payload, applied: [] };
    }
    const clonedOutput = clonePromptOutput(payload.output);
    const applied: ProximityMatch[] = [];
    for (const match of matches) {
      const originTuple: [string, number] = [String(match.originId), match.originSlot];
      let matchApplied = false;
      for (const target of match.promptTargets) {
        const nodeEntry = clonedOutput[String(target.nodeId)];
        if (!nodeEntry) continue;
        if (Array.isArray(nodeEntry.inputs)) {
          if (isConnection(nodeEntry.inputs[target.inputSlot])) continue;
          nodeEntry.inputs[target.inputSlot] = originTuple;
          matchApplied = true;
          continue;
        }
        nodeEntry.inputs ??= {};
        if (!isRecord(nodeEntry.inputs)) continue;
        const inputKey =
          target.inputName ||
          Object.keys(nodeEntry.inputs)[target.inputSlot] ||
          match.inputName ||
          'value';
        if (isConnection(nodeEntry.inputs[inputKey])) continue;
        nodeEntry.inputs[inputKey] = originTuple;
        matchApplied = true;
      }
      if (matchApplied) applied.push(match);
    }
    return applied.length > 0
      ? { payload: { ...payload, output: clonedOutput }, applied }
      : { payload, applied };
  }
}

/** Validate the minimum Comfy queue payload used by prompt-only linking. */
function isPromptPayload(value: unknown): value is PromptPayload {
  return isRecord(value) && isRecord(value.output);
}

/** Clone only prompt nodes and their input containers before writing routes. */
function clonePromptOutput(output: Record<string, PromptNode>): Record<string, PromptNode> {
  const result: Record<string, PromptNode> = {};
  for (const [key, value] of Object.entries(output)) {
    const entry: PromptNode = { ...value };
    if (Array.isArray(value.inputs)) {
      entry.inputs = value.inputs.map((input) => (Array.isArray(input) ? [...input] : input));
    } else if (isRecord(value.inputs)) {
      entry.inputs = Object.fromEntries(
        Object.entries(value.inputs).map(([name, input]) => [
          name,
          Array.isArray(input) ? [...input] : input,
        ]),
      );
    }
    result[key] = entry;
  }
  return result;
}

/** Identify Comfy's two-element execution connection tuple. */
function isConnection(value: unknown): boolean {
  return Array.isArray(value) && value.length === 2;
}
