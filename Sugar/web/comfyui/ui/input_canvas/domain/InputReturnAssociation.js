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
/** Attribute masked returns through the same input association policy used by consumer previews. */
import { resolveInputAssetAssociations, } from './InputAssetAssociation.js';
/** Resolve output provenance while retaining the authoritative mask-to-image association decisions. */
export function resolveInputReturnAssociations(graph, nodes) {
    const associations = resolveInputAssetAssociations([graph]);
    const byId = new Map(nodes.map((node) => [node.id, node]));
    const result = [];
    for (const output of nodes) {
        const masks = new Set(), composites = new Set(), visited = new Set();
        const pending = [output.id];
        while (pending.length) {
            const id = pending.pop();
            if (!id || visited.has(id))
                continue;
            visited.add(id);
            const node = byId.get(id);
            if (!node)
                continue;
            for (const mask of node.masks)
                masks.add(mask);
            for (const mask of node.composites)
                composites.add(mask);
            pending.push(...node.providers);
        }
        if (!masks.size)
            continue;
        const candidates = [...masks].map((mask) => associations.get(mask));
        const first = candidates[0];
        if (masks.size !== 1 || !first || first.kind !== 'associated') {
            result.push({
                kind: 'unavailable',
                output: output.id,
                message: 'The inpaint output does not have one unambiguous image and mask source.',
            });
            continue;
        }
        result.push({
            kind: 'associated',
            output: output.id,
            mask: first.mask,
            image: first.image,
            composited: composites.has(first.mask),
        });
    }
    return Object.freeze(result);
}
