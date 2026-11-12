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
/** Relate a regional mask loader to the region separators of the prompts it conditions. */

/** One related prompt's workflow path and the names of its separators, in source order. */
export interface RegionalPromptSeparators {
  readonly path: string;
  readonly separators: readonly (string | null)[];
}

/** Region names, the number of regions and the prompts that declare them. */
export interface RegionalPromptDemand {
  /** Name for region `i` (mask `i`), or null when no related prompt names it. */
  readonly names: readonly (string | null)[];
  /** The largest separator count among the related prompts. */
  readonly required: number;
  /** Workflow paths of the related prompts, in discovery order. */
  readonly prompts: readonly string[];
}

/**
 * Combine the separator names of each related prompt, in prompt order.
 *
 * `N` separators declare `N` regions after the global partition. Region `i` takes the first
 * non-blank name any prompt gives its separator `i`; the requirement is the largest count.
 */
export function regionalPromptDemand(
  prompts: readonly RegionalPromptSeparators[],
): RegionalPromptDemand {
  const required = Math.max(0, ...prompts.map(({ separators }) => separators.length));
  const names = Array.from({ length: required }, (_, index) => {
    for (const { separators } of prompts) {
      const name = separators[index]?.trim();
      if (name) return name;
    }
    return null;
  });
  return { names, required, prompts: prompts.map(({ path }) => path) };
}

/** Count regions a prompt declares beyond the masks a loader holds. */
export function missingRegionMasks(demand: RegionalPromptDemand | null, masks: number): number {
  return demand ? Math.max(0, demand.required - masks) : 0;
}
