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
/** Resolve native Cube boundary types from the internal slots they expose. */
/** Return one exact shared link type, falling back to wildcard for ambiguous host data. */
export function resolveCubeInputBoundaryType(slots) {
    const types = slots.map((slot) => readPortType(slot.type));
    const first = types[0];
    if (!first || types.some((type) => type !== first))
        return '*';
    return first;
}
/** Normalize one Comfy link type without treating widget choice arrays as socket types. */
function readPortType(value) {
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}
