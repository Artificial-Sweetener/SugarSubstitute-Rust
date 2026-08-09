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
/** Project backend revision history into unique selectable Cube versions. */
import { CURRENT_REVISION_REF, formatCubeVersionLabel, isCurrentRevisionRef, normalizeCubeVersion, normalizeRevisionRef, } from '../../core/CubeDefinitionKey.js';
/** Build one option per semantic version while retaining exact revision identity. */
export function projectCubeVersionOptions(revisions, fallbackVersion) {
    const options = revisions.flatMap((revision) => {
        const version = normalizeCubeVersion(revision.version);
        const revisionRef = normalizeRevisionRef(revision.revision_ref);
        if (!version || !revisionRef)
            return [];
        return [
            {
                label: formatCubeVersionLabel(version),
                value: version,
                revisionRef,
                current: isCurrentRevisionRef(revisionRef),
                raw: revision,
            },
        ];
    });
    if (options.length > 0)
        return options;
    const version = normalizeCubeVersion(fallbackVersion);
    return version
        ? [
            {
                label: formatCubeVersionLabel(version),
                value: version,
                revisionRef: CURRENT_REVISION_REF,
                current: true,
                raw: null,
            },
        ]
        : [];
}
