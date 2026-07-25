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
/** Restore saved CubeOutput execution behavior after Comfy flattens native boundaries. */
import { isRecord } from '../../types/common.js';
import { requireCubeIdentity } from '../node/ComfyCubeNodeFactory.js';
import { buildCubeOutputExecutionId } from './CubeOutputExecutionIdentity.js';
/** Own runtime-only projection of persisted output markers into prompt sinks. */
export class CubeOutputPromptAdapter {
    #getCubes;
    #boundaryResolver;
    /** Bind the current native Cube inventory and shared boundary resolver. */
    constructor(options) {
        this.#getCubes = options.getCubes;
        this.#boundaryResolver = options.boundaryResolver;
    }
    /** Add one non-persisted CubeOutput sink for every loaded output boundary. */
    apply(payloadValue) {
        if (!isRecord(payloadValue) || !isRecord(payloadValue.output))
            return payloadValue;
        const additions = new Map();
        for (const cube of this.#getCubes()) {
            if (cube.id == null)
                continue;
            const identity = readExecutionIdentity(cube);
            for (let slot = 0; slot < cube.subgraph.outputs.length; slot += 1) {
                const source = this.#boundaryResolver.resolveOutput(cube, slot);
                if (!source) {
                    throw new Error(`Cube '${String(cube.id)}' output ${String(slot)} has no executable source.`);
                }
                const executionId = buildCubeOutputExecutionId(cube.id, slot);
                if (executionId in payloadValue.output || additions.has(executionId)) {
                    throw new Error(`Cube output execution id '${executionId}' collides with a prompt node.`);
                }
                const output = cube.subgraph.outputs[slot];
                additions.set(executionId, {
                    inputs: {
                        value: [String(source.nodeId), source.slot],
                        cube_id: identity.cubeId,
                        default_alias: identity.defaultAlias,
                        instance_alias: identity.instanceAlias,
                        instance_id: identity.instanceId,
                    },
                    class_type: 'SugarCubes.CubeOutput',
                    _meta: { title: readOutputName(output?.name, slot) },
                });
            }
        }
        if (additions.size === 0)
            return payloadValue;
        return {
            ...payloadValue,
            output: {
                ...payloadValue.output,
                ...Object.fromEntries(additions),
            },
        };
    }
}
/** Require marker metadata that existing `.cube` files already provide at placement. */
function readExecutionIdentity(cube) {
    const identity = requireCubeIdentity(cube);
    const cubeId = requireString(identity.cube_id, 'cube_id', cube);
    const defaultAlias = requireString(identity.default_alias, 'default_alias', cube);
    const instanceId = requireString(identity.instance_id, 'instance_id', cube);
    const instanceAlias = readString(identity.instance_alias) || defaultAlias;
    return { cubeId, defaultAlias, instanceAlias, instanceId };
}
/** Require one non-empty persisted Cube identity field. */
function requireString(value, field, cube) {
    const result = readString(value);
    if (!result) {
        throw new TypeError(`Cube '${String(cube.id)}' is missing required '${field}' metadata.`);
    }
    return result;
}
/** Read one non-empty external string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Preserve saved output order while supplying a stable diagnostic fallback. */
function readOutputName(value, slot) {
    return readString(value) || `output.${String(slot)}`;
}
