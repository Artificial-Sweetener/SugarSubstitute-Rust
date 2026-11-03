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
/**
 * Combine the separator names of each related prompt, in prompt order.
 *
 * `N` separators declare `N` regions after the global partition. Region `i` takes the first
 * non-blank name any prompt gives its separator `i`; the requirement is the largest count.
 */
export function regionalPromptDemand(prompts) {
    const required = Math.max(0, ...prompts.map((separators) => separators.length));
    const names = Array.from({ length: required }, (_, index) => {
        for (const separators of prompts) {
            const name = separators[index]?.trim();
            if (name)
                return name;
        }
        return null;
    });
    return { names, required };
}
/** Count regions a prompt declares beyond the masks a loader holds. */
export function missingRegionMasks(demand, masks) {
    return demand ? Math.max(0, demand.required - masks) : 0;
}
