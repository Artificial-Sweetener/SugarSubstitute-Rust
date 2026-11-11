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
/** Project rail rows from the mounted masonry inventory and canonical graph permissions. */
import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeIdentityPresentation, } from '../../cube/CubeIdentityPresentation.js';
import { orderCanonicalCubeSections } from '../masonry/WorkflowMasonryProjection.js';
/** Present compact stack identity while retaining canonical section order and full tooltips. */
export function projectCubeStackRows(sections, analysis, selectedId, resolveSource, available) {
    return orderCanonicalCubeSections(sections, analysis).flatMap((section) => {
        const node = section.cube;
        if (!node)
            return [];
        const metadata = requireCubeIdentity(node);
        const id = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
        if (!id)
            throw new Error('Mounted Cube stack section has no stable instance identity.');
        const identity = resolveCubeIdentityPresentation({
            metadata,
            instanceTitle: node.title ?? '',
            fallbackDefinitionTitle: node.subgraph.name,
            fallbackSource: resolveSource(metadata),
        });
        return [
            {
                id,
                label: identity.instanceModelTitle.nameText,
                secondary: [identity.versionText, formatStackPackText(metadata.cube_id)]
                    .filter(Boolean)
                    .join(' · '),
                tooltip: [identity.instanceTitle, identity.definitionLine, identity.sourceLine]
                    .filter(Boolean)
                    .join('\n'),
                icon: identity.icon,
                targetModel: identity.targetModel,
                bypassed: node.mode === 4,
                severity: identity.awaitingFirstSave ? 'warning' : 'none',
                selected: id === selectedId,
                reorderable: available,
            },
        ];
    });
}
/** Show the compact repository or local namespace retained by canonical and legacy Cube ids. */
function formatStackPackText(cubeId) {
    if (typeof cubeId !== 'string')
        return '';
    const parts = cubeId
        .replace(/\\/g, '/')
        .split('/')
        .map((part) => part.trim())
        .filter(Boolean);
    if (parts.length < 2)
        return '';
    const pack = parts.length >= 3 ? parts[1] : parts[0];
    return (pack ?? '')
        .replace(/[\s_]+/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '')
        .toLowerCase();
}
