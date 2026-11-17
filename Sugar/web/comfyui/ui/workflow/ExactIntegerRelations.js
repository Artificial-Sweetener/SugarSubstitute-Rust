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
/** Keep the digits of whole numbers a page cannot hold, on the workflow that uses them. */
import { isRecord } from '../types/common.js';
/** Workflow `extra` key holding workflow-level value relations the backend applies when it runs. */
const COMPOSITION_KEY = 'sugarcubes_composition';
/** Validate the backend's list; anything that is not a named field with digits is dropped. */
export function readExactIntegerFields(value) {
    if (!Array.isArray(value))
        return [];
    return value.flatMap((item) => {
        if (!isRecord(item) ||
            typeof item.instance_id !== 'string' ||
            typeof item.node_symbol !== 'string' ||
            typeof item.input_name !== 'string' ||
            typeof item.digits !== 'string' ||
            !/^-?(?:0|[1-9][0-9]{0,39})$/u.test(item.digits))
            return [];
        return [
            {
                instanceId: item.instance_id,
                nodeSymbol: item.node_symbol,
                inputName: item.input_name,
                digits: item.digits,
            },
        ];
    });
}
/**
 * Record each field's digits as an `exact_integer` relation in the workflow's own metadata.
 *
 * A 64-bit seed reaches this page rounded, and the page sends that rounding when it queues. The
 * backend reads these relations as it runs the workflow and gives a field its recorded digits
 * while the field still holds exactly that rounding, so the run uses the original seed. A field
 * the person has changed holds another number and keeps it. The relations travel with the
 * workflow, including in the images it generates.
 */
export function recordExactIntegers(root, fields) {
    if (!fields.length)
        return;
    const held = Reflect.get(root, 'extra');
    const extra = isRecord(held) ? held : {};
    const composition = isRecord(extra[COMPOSITION_KEY]) ? extra[COMPOSITION_KEY] : {};
    const existing = Array.isArray(composition.value_relations)
        ? composition.value_relations
        : [];
    const recorded = fields.map((field) => ({
        relation_id: `exact:${field.instanceId}:${field.nodeSymbol}:${field.inputName}`,
        kind: 'exact_integer',
        digits: field.digits,
        targets: [
            {
                instance_id: field.instanceId,
                node_symbol: field.nodeSymbol,
                input_name: field.inputName,
            },
        ],
    }));
    const replaced = new Set(recorded.map((relation) => relation.relation_id));
    Reflect.set(root, 'extra', {
        ...extra,
        [COMPOSITION_KEY]: {
            ...composition,
            schema_version: 1,
            value_relations: [
                ...existing.filter((relation) => !isRecord(relation) || !replaced.has(String(relation.relation_id))),
                ...recorded,
            ],
        },
    });
}
