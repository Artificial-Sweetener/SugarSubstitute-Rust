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
/** Describe attributable workflow media without decoded images or host values. */
import { outputSourceId, } from './ExecutionIdentity.js';
/**
 * Reduce execution-path attributions to one entry per output source.
 *
 * A Cube routes every node inside it to its exposed output so their previews appear there, which
 * gives several paths one source identity. Only the path that delivers the source's finals
 * describes the source itself: the path the identity names, else a path whose finals it accepts.
 */
export function finalOutputSources(values) {
    const chosen = new Map();
    for (const value of values) {
        const current = chosen.get(value.id);
        if (!current || finalPathRank(value) < finalPathRank(current))
            chosen.set(value.id, value);
    }
    return Object.freeze([...chosen.values()]);
}
/** Order one source's paths by how directly each delivers its final media. */
function finalPathRank(source) {
    if (source.id === outputSourceId(source.executionPath))
        return 0;
    return source.transientOnly ? 2 : 1;
}
