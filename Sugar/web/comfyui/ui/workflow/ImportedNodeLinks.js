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
/** Read the node links a workflow saved by the Python application records for its Cubes. */
import { isRecord } from '../types/common.js';
/** Name one Cube node the way workflow relations identify it. */
function nodeIdentity(endpoint) {
    if (!isRecord(endpoint))
        return null;
    const { instance_id: instance, node_symbol: symbol } = endpoint;
    return typeof instance === 'string' && typeof symbol === 'string'
        ? JSON.stringify(['cube', instance, symbol])
        : null;
}
/**
 * Return the links the workflow records, or `null` for a workflow that records none of its own.
 * A workflow with a composition and no link in it records that every node is independent.
 */
export function readImportedNodeLinks(workflow) {
    const composition = isRecord(workflow.extra) ? workflow.extra.sugarcubes_composition : null;
    if (!isRecord(composition))
        return null;
    const relations = Array.isArray(composition.value_relations) ? composition.value_relations : [];
    const links = new Map();
    for (const relation of relations) {
        if (!isRecord(relation) || relation.kind !== 'field_link')
            continue;
        const source = nodeIdentity(relation.source);
        if (!source || !Array.isArray(relation.targets))
            continue;
        for (const endpoint of relation.targets) {
            const target = nodeIdentity(endpoint);
            if (target && target !== source)
                links.set(target, source);
        }
    }
    return [...links].map(([target, source]) => ({ target, source }));
}
