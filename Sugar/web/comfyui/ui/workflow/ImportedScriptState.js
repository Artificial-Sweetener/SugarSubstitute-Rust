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
/** Read the editor state a project script's compiled workflow records beside its values. */
import { isRecord } from '../types/common.js';
const COMPOSITION_KEY = 'sugarcubes_composition';
/** Composition entries a script's workflow carries for its writer, which no page owner edits. */
const CARRIED = ['field_annotations', 'script_state'];
/** Name one Cube input the way workflow relations identify a field. */
function fieldIdentity(endpoint) {
    if (!isRecord(endpoint))
        return null;
    const { instance_id: instance, node_symbol: symbol, input_name: input } = endpoint;
    return typeof instance === 'string' && typeof symbol === 'string' && typeof input === 'string'
        ? JSON.stringify([JSON.stringify(['cube', instance, symbol]), input])
        : null;
}
/** Tell whether a value is one a relation field can hold. */
function isValue(value) {
    return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';
}
/** Read a script's composition, or `null` for a workflow that carries none. */
function composition(workflow) {
    const held = isRecord(workflow.extra) ? workflow.extra[COMPOSITION_KEY] : null;
    return isRecord(held) ? held : null;
}
/** Read the overrides, own values and fixed seeds one compiled script records. */
export function readImportedScriptRelations(workflow) {
    const held = composition(workflow);
    const relations = Array.isArray(held?.value_relations) ? held.value_relations : [];
    const overrides = [];
    const ownValues = [];
    for (const relation of relations) {
        if (!isRecord(relation))
            continue;
        if (relation.kind === 'global_override') {
            if (typeof relation.key === 'string' && isValue(relation.value))
                overrides.push({ key: relation.key, value: relation.value });
        }
        else if (relation.kind === 'field_link' && Array.isArray(relation.targets)) {
            for (const endpoint of relation.targets) {
                const field = fieldIdentity(endpoint);
                if (field && isValue(relation.own_value))
                    ownValues.push({ field, value: relation.own_value });
            }
        }
    }
    const state = isRecord(held?.script_state) ? held.script_state : {};
    const fixedOverrides = Array.isArray(state.fixed_seed_overrides)
        ? state.fixed_seed_overrides
        : [];
    const fixedFields = Array.isArray(state.fixed_seed_fields)
        ? state.fixed_seed_fields
        : [];
    return {
        overrides,
        fixedSeedOverrides: fixedOverrides.filter((key) => typeof key === 'string'),
        fixedSeedFields: fixedFields.flatMap((endpoint) => fieldIdentity(endpoint) ?? []),
        ownValues,
    };
}
/** Name the Cube instances whose outputs a compiled script says are not saved. */
export function readUnsavedOutputInstances(workflow) {
    const nodes = Array.isArray(workflow.nodes) ? workflow.nodes : [];
    return nodes.flatMap((node) => {
        const properties = isRecord(node) && isRecord(node.properties) ? node.properties : null;
        const cube = properties && isRecord(properties.sugarcubes_cube) ? properties.sugarcubes_cube : null;
        return properties?.sugarcubes_save_outputs === false && typeof cube?.instance_id === 'string'
            ? [cube.instance_id]
            : [];
    });
}
/** Mark the named Cube instances of a workflow as not saving their outputs. */
export function markUnsavedOutputs(root, instanceIds) {
    if (!instanceIds.length)
        return;
    const wanted = new Set(instanceIds);
    const held = Reflect.get(root, '_nodes') ?? Reflect.get(root, 'nodes');
    for (const node of Array.isArray(held) ? held : []) {
        const properties = isRecord(node) && isRecord(node.properties) ? node.properties : null;
        const cube = properties && isRecord(properties.sugarcubes_cube) ? properties.sugarcubes_cube : null;
        if (properties && typeof cube?.instance_id === 'string' && wanted.has(cube.instance_id))
            properties.sugarcubes_save_outputs = false;
    }
}
/**
 * Keep, on the workflow being built, what a script's writer needs and no page owner edits:
 * the model and LoRA hashes beneath its fields and the script state nothing on the page shows.
 * The page's own records in the same metadata are left as they are.
 */
export function carryScriptComposition(root, workflow) {
    const source = composition(workflow);
    if (!source)
        return;
    const carried = Object.fromEntries(CARRIED.flatMap((name) => name in source ? [[name, JSON.parse(JSON.stringify(source[name]))]] : []));
    if (!Object.keys(carried).length)
        return;
    const held = Reflect.get(root, 'extra');
    const extra = isRecord(held) ? held : {};
    const existing = isRecord(extra[COMPOSITION_KEY]) ? extra[COMPOSITION_KEY] : {};
    Reflect.set(root, 'extra', {
        ...extra,
        [COMPOSITION_KEY]: { ...existing, schema_version: 1, ...carried },
    });
}
