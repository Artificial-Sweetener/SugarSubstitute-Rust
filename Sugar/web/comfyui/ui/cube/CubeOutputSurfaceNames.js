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
/** Derive concise Cube-interface output names from canonical Sugar bindings. */
const OUTPUT_BINDING_PREFIX = 'output.';
/** Return one human-facing name for each Cube output without changing DSL bindings. */
export function deriveCubeOutputSurfaceNames(outputs) {
    const explicitNames = outputs.map(readExplicitSurfaceName);
    const reservedNames = new Set(explicitNames.filter((name) => name !== null));
    const counts = new Map();
    return outputs.map((output, index) => {
        const explicitName = explicitNames[index];
        if (explicitName)
            return explicitName;
        return allocateTypeName(output.type, counts, reservedNames);
    });
}
/** Preserve a deliberate name while converting a canonical Sugar binding for display. */
function readExplicitSurfaceName(output) {
    const name = readString(output.name);
    if (!name || isGenericTypeName(name, output.type))
        return null;
    if (!name.startsWith(OUTPUT_BINDING_PREFIX))
        return name;
    const surfaceName = name.slice(OUTPUT_BINDING_PREFIX.length).trim();
    return surfaceName || null;
}
/** Allocate a stable suffix for an automatically named output of one type. */
function allocateTypeName(type, counts, reservedNames) {
    const base = typeNameBase(type);
    let count = counts.get(base) ?? 0;
    let candidate = '';
    do {
        count += 1;
        candidate = count === 1 ? base : `${base}${String(count)}`;
    } while (reservedNames.has(candidate));
    counts.set(base, count);
    return candidate;
}
/** Identify Comfy's default type-only output label. */
function isGenericTypeName(name, type) {
    return name.toLocaleLowerCase() === typeNameBase(type);
}
/** Build a readable, identifier-safe default from a Comfy output type. */
function typeNameBase(type) {
    const text = readString(type)
        .toLocaleLowerCase()
        .replace(/[^a-z0-9]+/g, '_')
        .replace(/^_+|_+$/g, '');
    return text && text !== 'any' ? text : 'value';
}
/** Read one non-empty string from a host-owned value. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
