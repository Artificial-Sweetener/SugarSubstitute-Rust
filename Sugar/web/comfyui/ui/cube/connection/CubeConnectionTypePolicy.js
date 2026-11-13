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
/** Own Comfy-compatible type validation for every Cube boundary connection. */
/** Delegate connection compatibility to Comfy while retaining deterministic fallback semantics. */
export class CubeConnectionTypePolicy {
    #getLiteGraph;
    #logger;
    /** Bind the active Comfy type system at its dynamic host boundary. */
    constructor(options) {
        this.#getLiteGraph = options.getLiteGraph;
        this.#logger = options.logger;
    }
    /** Return whether an output may connect to an input under current Comfy semantics. */
    accepts(outputType, inputType, strict = false) {
        const output = normalizeCubePortType(outputType);
        const input = normalizeCubePortType(inputType);
        try {
            const liteGraph = this.#getLiteGraph();
            if (liteGraph?.isValidConnection) {
                if (strict) {
                    if (output === '*' || input === '*')
                        return false;
                    return (liteGraph.isValidConnection(output, input) && liteGraph.isValidConnection(input, output));
                }
                return liteGraph.isValidConnection(output, input);
            }
        }
        catch (error) {
            this.#logger.debug('SugarCubes Cube boundary type check failed.', {
                outputType: output,
                inputType: input,
                error,
            });
            return false;
        }
        if (strict)
            return output !== '*' && input !== '*' && output === input;
        return output === '*' || input === '*' || output === input;
    }
}
/** Normalize one dynamic Comfy slot type for stable comparisons and persistence. */
export function normalizeCubePortType(type) {
    if (type == null || type === '')
        return '*';
    const normalized = String(type).trim().toUpperCase();
    return normalized && normalized !== '*' ? normalized : '*';
}
