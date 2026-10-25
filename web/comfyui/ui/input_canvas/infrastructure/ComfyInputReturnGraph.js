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
/** Validate compiled Comfy connections and translate them into shared return-association facts. */
import { isRecord } from '../../types/common.js';
import { resolveInputReturnAssociations } from '../domain/InputReturnAssociation.js';
/** Resolve only validated graph links, excluding literal lists supplied through native multiselect wrappers. */
function link(value) {
    return Array.isArray(value) &&
        value.length === 2 &&
        typeof value[0] === 'string' &&
        typeof value[1] === 'number' &&
        Number.isSafeInteger(value[1]) &&
        value[1] >= 0
        ? { node: value[0], slot: value[1] }
        : null;
}
/** Compare native scalar and ordered upload values without accepting paths from output metadata. */
function fileValues(value) {
    const supplied = isRecord(value) && Object.hasOwn(value, '__value__') ? value.__value__ : value;
    return typeof supplied === 'string'
        ? [supplied]
        : Array.isArray(supplied) && supplied.every((v) => typeof v === 'string')
            ? supplied
            : [];
}
/** Preserve captured field identities through compiler-generated paths using exact immutable upload evidence. */
export function compiledInputReturnAssociations(prompt, captured, ports) {
    if (!isRecord(prompt))
        throw new Error('The queued inpaint has no canonical execution graph.');
    const nodes = new Map(Object.entries(prompt).filter((entry) => isRecord(entry[1]) && typeof entry[1].class_type === 'string' && isRecord(entry[1].inputs)));
    const endpoints = new Map();
    for (const [id, node] of nodes) {
        const inputs = isRecord(node.inputs) ? node.inputs : {};
        const matches = captured.filter((input) => input.classType === node.class_type &&
            JSON.stringify(input.files) === JSON.stringify(fileValues(inputs[input.field])));
        const exact = matches.filter((input) => input.nodePath === id);
        endpoints.set(id, exact.length ? exact : matches);
    }
    /** Follow spatial inputs through mask/image processing nodes while leaving their policy to normalized associations. */
    const roots = (id, slot, kind, visited) => {
        if (visited.has(id))
            return [];
        const node = nodes.get(id);
        if (!node ||
            typeof node.class_type !== 'string' ||
            ports.get(node.class_type)?.outputs[slot] !== kind)
            return [];
        const direct = (endpoints.get(id) ?? []).filter((input) => input.kind === kind);
        if (direct.length)
            return [...new Set(direct.map((input) => input.key))];
        const inputs = isRecord(node.inputs) ? node.inputs : {};
        const result = [];
        for (const [name, type] of Object.entries(ports.get(node.class_type)?.inputs ?? {})) {
            const provider = link(inputs[name]);
            if (type === kind && provider)
                result.push(...roots(provider.node, provider.slot, kind, new Set([...visited, id])));
        }
        return [...new Set(result)];
    };
    const inputs = [], topology = [];
    for (const [id, node] of nodes) {
        const values = isRecord(node.inputs) ? node.inputs : {};
        const composites = [];
        const incomingMasks = [];
        for (const [name, kind] of Object.entries(typeof node.class_type === 'string' ? (ports.get(node.class_type)?.inputs ?? {}) : {})) {
            const provider = link(values[name]);
            if (!provider)
                continue;
            const found = roots(provider.node, provider.slot, kind, new Set());
            if (kind === 'mask') {
                incomingMasks.push(...found);
                if (!found.length &&
                    captured.some((input) => input.kind === 'mask' && input.classType === nodes.get(provider.node)?.class_type))
                    incomingMasks.push(`unresolved:${provider.node}:${provider.slot}`);
            }
            // The generated source is the replacement; the destination supplies the composite's image context.
            if (node.class_type !== 'ImageCompositeMasked' || name !== 'source')
                inputs.push({ consumer: id, input: name, kind, endpoints: found });
            if (node.class_type === 'ImageCompositeMasked' &&
                name === 'mask' &&
                (endpoints.get(provider.node) ?? []).some((input) => input.kind === 'mask'))
                composites.push(...found);
        }
        const direct = endpoints.get(id) ?? [];
        const masks = [
            ...incomingMasks,
            ...(direct.some((input) => input.kind === 'image')
                ? []
                : direct.filter((input) => input.kind === 'mask').map((input) => input.key)),
        ];
        if (!direct.length &&
            !(typeof node.class_type === 'string' && ports.get(node.class_type)?.outputs.includes('image')) &&
            captured.some((input) => input.kind === 'mask' && input.classType === node.class_type))
            masks.push(`unresolved:${id}`);
        topology.push({
            id,
            providers: Object.values(values).flatMap((value) => {
                const edge = link(value);
                return edge ? [edge.node] : [];
            }),
            masks,
            composites,
        });
    }
    const graph = {
        endpoints: captured.map(({ key, kind }) => ({ key, kind })),
        inputs,
    };
    return resolveInputReturnAssociations(graph, topology);
}
