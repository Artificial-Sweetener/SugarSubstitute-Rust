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
/** How many digit records one input keeps; older ones belong to seeds long since replaced. */
const RECORDS_PER_INPUT = 8;
/**
 * Record each field's digits as an `exact_integer` relation in the workflow's own metadata.
 *
 * A 64-bit seed reaches this page rounded, and the page sends that rounding when it queues. The
 * backend reads these relations as it runs the workflow and gives a field its recorded digits
 * while the field still holds exactly that rounding, so the run uses the original seed. A field
 * the person has changed holds another number and keeps it. The relations travel with the
 * workflow, including in the images it generates.
 *
 * An input keeps one record per rounding. A shared seed and a Cube's own seed are two values
 * the same input alternates between, and each keeps its digits; digits recorded for a rounding
 * replace the ones that rounding had.
 */
export function recordExactIntegers(root, fields) {
    if (!fields.length)
        return;
    const held = Reflect.get(root, 'extra');
    const extra = isRecord(held) ? held : {};
    const composition = isRecord(extra[COMPOSITION_KEY]) ? extra[COMPOSITION_KEY] : {};
    let kept = Array.isArray(composition.value_relations)
        ? composition.value_relations
        : [];
    for (const field of fields) {
        const rounding = Number(field.digits);
        const others = kept.filter((relation) => !records(relation, field));
        const own = kept.filter((relation) => records(relation, field) && Number(relation.digits) !== rounding);
        kept = [
            ...others,
            ...own.slice(-(RECORDS_PER_INPUT - 1)),
            {
                relation_id: `exact:${field.instanceId}:${field.nodeSymbol}:${field.inputName}:${field.digits}`,
                kind: 'exact_integer',
                digits: field.digits,
                targets: [
                    {
                        instance_id: field.instanceId,
                        node_symbol: field.nodeSymbol,
                        input_name: field.inputName,
                    },
                ],
            },
        ];
    }
    Reflect.set(root, 'extra', {
        ...extra,
        [COMPOSITION_KEY]: { ...composition, schema_version: 1, value_relations: kept },
    });
}
/** Return the digits recorded for the value one input holds, or null when it has none. */
export function readExactDigits(root, target, value) {
    const extra = Reflect.get(root, 'extra');
    const composition = isRecord(extra) ? extra[COMPOSITION_KEY] : null;
    const relations = isRecord(composition) && Array.isArray(composition.value_relations)
        ? composition.value_relations
        : [];
    for (const relation of relations)
        if (records(relation, target) && Number(relation.digits) === value)
            return relation.digits;
    return null;
}
/** Tell whether a relation is the digit record of exactly this one input. */
function records(relation, target) {
    if (!isRecord(relation) || relation.kind !== 'exact_integer')
        return false;
    if (typeof relation.digits !== 'string' || !Array.isArray(relation.targets))
        return false;
    const targets = relation.targets;
    const only = targets[0];
    return (targets.length === 1 &&
        isRecord(only) &&
        only.instance_id === target.instanceId &&
        only.node_symbol === target.nodeSymbol &&
        only.input_name === target.inputName);
}
