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
/** Resolve the shared instance and definition identity shown by Cube chrome. */
import { resolveCubeIconModel } from '../core/CubeIconResolver.js';
import { parseCanonicalCubeId } from '../core/CubeId.js';
import { resolveCubeDisplayName, resolveInstanceDisplayName, } from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
/** Format one persisted Cube version with the former group-chrome wording. */
export function formatCubeVersionText(metadata) {
    const version = typeof metadata.cube_version === 'string' ? metadata.cube_version.trim() : '';
    if (!version)
        return '';
    return `version ${version.replace(/^[vV](?=\d)/, '')}`;
}
/** Format the former group-chrome source line from canonical Cube identity. */
export function formatCubeSourceText(metadata, fallbackSource = null) {
    const cubeId = typeof metadata.cube_id === 'string' ? metadata.cube_id.trim() : '';
    if (cubeId) {
        try {
            const parsed = parseCanonicalCubeId(cubeId);
            if (parsed.sourceKind === 'github') {
                return `from ${parsed.repo} by ${parsed.owner}`;
            }
            return parsed.namespace ? `from local ${parsed.namespace}` : 'from local';
        }
        catch (_error) {
            // Presentation remains available for legacy or malformed persisted identifiers.
        }
    }
    const pack = typeof fallbackSource?.pack === 'string' ? fallbackSource.pack.trim() : '';
    const author = typeof fallbackSource?.author === 'string' ? fallbackSource.author.trim() : '';
    const namespace = typeof fallbackSource?.namespace === 'string' ? fallbackSource.namespace.trim() : '';
    if (fallbackSource?.sourceKind === 'local') {
        return namespace ? `from local ${namespace}` : 'from local';
    }
    if (pack && author)
        return `from ${pack} by ${author}`;
    if (pack)
        return `from ${pack}`;
    if (author)
        return `by ${author}`;
    return 'from Unknown';
}
/** Build the one identity model consumed by canvas and DOM Cube headers. */
export function resolveCubeIdentityPresentation(input) {
    const metadata = (isRecord(input.metadata) ? input.metadata : {});
    const fallbackDefinitionTitle = input.fallbackDefinitionTitle.trim() || 'SugarCube';
    const definitionTitle = resolveCubeDisplayName({
        metadata,
        fallback: fallbackDefinitionTitle,
    });
    const explicitInstanceTitle = input.instanceTitle.trim();
    const instanceTitle = explicitInstanceTitle ||
        resolveInstanceDisplayName({
            metadata,
            fallback: definitionTitle,
        });
    const versionText = formatCubeVersionText(metadata);
    return {
        instanceTitle,
        definitionTitle,
        versionText,
        definitionLine: versionText ? `${definitionTitle} ${versionText}` : definitionTitle,
        sourceLine: formatCubeSourceText(metadata, input.fallbackSource ?? null),
        icon: resolveCubeIconModel(metadata),
    };
}
