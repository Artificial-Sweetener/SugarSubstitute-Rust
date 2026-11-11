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
/** Project temporary Cube section order without changing loose runs, graph data or native controls. */
/** Replace only existing Cube slots after validating an exact display permutation. */
export function previewMasonrySections(sections, cubeSectionIds) {
    if (cubeSectionIds === null)
        return sections;
    const cubes = new Map(sections.filter((section) => section.cube !== null).map((section) => [section.id, section]));
    if (cubeSectionIds.length !== cubes.size ||
        new Set(cubeSectionIds).size !== cubeSectionIds.length ||
        cubeSectionIds.some((id) => !cubes.has(id)))
        throw new TypeError('Masonry preview must be an exact Cube section permutation.');
    let index = 0;
    return sections.map((section) => {
        if (section.cube === null)
            return section;
        const id = cubeSectionIds[index++];
        const replacement = id === undefined ? undefined : cubes.get(id);
        if (!replacement)
            throw new Error('Masonry preview section disappeared during a synchronous projection.');
        return replacement;
    });
}
