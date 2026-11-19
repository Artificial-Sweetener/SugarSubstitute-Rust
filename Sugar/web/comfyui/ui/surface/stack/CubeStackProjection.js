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
import { cubeStackRowIdentity } from './CubeStackRowIdentity.js';
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
                ...cubeStackRowIdentity(identity, metadata.cube_id),
                id,
                bypassed: node.mode === 4,
                selected: id === selectedId,
                reorderable: available,
            },
        ];
    });
}
