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
/** Build canonical workflow requests for authoritative SugarCubes execution. */
import { isCubeKind } from '../CubeKind.js';
import { enrichWorkflowPayload } from '../../graph/WorkflowPayloadBuilder.js';
import { isRecord } from '../../types/common.js';
import { CubeExecutionApiClient } from './CubeExecutionApiClient.js';
/** Own browser-side request projection without interpreting Cube graph semantics. */
export class CubeDirectExecutionService {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Return whether a queue payload belongs to native SugarCubes execution. */
    owns(payload) {
        if (!isRecord(payload) || !isRecord(payload.workflow))
            return false;
        if (isRecord(payload.extra_data) && isRecord(payload.extra_data.substitute))
            return false;
        const nodes = payload.workflow.nodes;
        return Array.isArray(nodes) && nodes.some(isPersistedCubeNode);
    }
    /** Serialize live workflow state and delegate execution to SugarCubes once. */
    async queue(position, comfyPayload, queueOptions = {}, preparedInput) {
        const graph = this.options.getGraph();
        if (!graph)
            throw new TypeError('The active Comfy graph is unavailable.');
        const serialize = graph.serialize;
        if (typeof serialize !== 'function') {
            throw new TypeError('The active Comfy graph cannot be serialized.');
        }
        const stack = this.options.getStack() ?? { order: [], proximity_connections: [] };
        const serialized = enrichWorkflowPayload(serialize.call(graph), graph);
        const scene = this.options.sceneWorkflow
            ? this.options.sceneWorkflow(comfyPayload, serialized)
            : serialized;
        const authored = this.options.resolveWorkflow
            ? await this.options.resolveWorkflow(comfyPayload, scene)
            : scene;
        const preparation = this.options.prepareInputs
            ? await this.options.prepareInputs({ workflow: authored })
            : { payload: { workflow: authored } };
        const prepared = preparation.payload;
        if (!isRecord(prepared) || !isRecord(prepared.workflow))
            throw new Error('Canvas input preparation returned an invalid workflow.');
        const workflow = prepared.workflow;
        preparedInput?.(preparation);
        const payload = isRecord(comfyPayload) ? comfyPayload : {};
        const extraData = buildExtraData(workflow, payload, queueOptions, this.options.hostApi);
        // Output saving numbers images by the stack the page shows.
        extraData.sugarcubes_stack = stack;
        const clientId = readString(this.options.hostApi?.clientId) || readString(payload.client_id);
        const explicitNumber = position !== 0 && position !== -1 ? position : null;
        this.options.logger?.debug('SugarCubes sent the stack with a direct run.', {
            cubes: stack.order.length,
            proximityConnections: stack.proximity_connections.length,
        });
        return await this.options.client.queue({
            ...(preparation.preparedInputs?.length
                ? { prepared_inputs: preparation.preparedInputs }
                : {}),
            schema_version: 1,
            workflow,
            proximity_connections: [...stack.proximity_connections],
            queue: {
                client_id: clientId || null,
                front: position === -1,
                number: explicitNumber,
                partial_execution_targets: readStringArray(queueOptions.partialExecutionTargets ?? payload.partial_execution_targets),
                atomic: true,
            },
            optimization: {
                enabled: true,
                bypass_empty_lazy_lora: true,
                intern_pure_values: true,
                intern_resource_streams: true,
            },
            extra_data: extraData,
        });
    }
}
/** Compose direct execution without expanding the root UI composition owner. */
export function createCubeDirectExecution(options) {
    return new CubeDirectExecutionService({
        client: new CubeExecutionApiClient(options.api),
        hostApi: options.api,
        ...(options.logger ? { logger: options.logger } : {}),
        getGraph: options.getGraph,
        getStack: options.getStack,
        ...(options.prepareInputs ? { prepareInputs: options.prepareInputs } : {}),
        ...(options.sceneWorkflow ? { sceneWorkflow: options.sceneWorkflow } : {}),
        ...(options.resolveWorkflow ? { resolveWorkflow: options.resolveWorkflow } : {}),
    });
}
/** Preserve native Comfy history and sensitive metadata for direct queueing. */
function buildExtraData(workflow, payload, queueOptions, hostApi) {
    const result = isRecord(payload.extra_data) ? { ...payload.extra_data } : {};
    result.comfy_usage_source = 'comfyui-frontend';
    result.extra_pnginfo = { workflow };
    copyNonEmptyString(result, 'auth_token_comfy_org', hostApi?.authToken);
    copyNonEmptyString(result, 'api_key_comfy_org', hostApi?.apiKey);
    const previewMethod = readString(queueOptions.previewMethod);
    if (previewMethod && previewMethod !== 'default')
        result.preview_method = previewMethod;
    return result;
}
/** Copy one secret-bearing Comfy value only when the host supplied text. */
function copyNonEmptyString(target, key, value) {
    const normalized = readString(value);
    if (normalized)
        target[key] = normalized;
}
/** Identify a durable native Cube marker without resolving its definition. */
function isPersistedCubeNode(value) {
    if (!isRecord(value) || !isRecord(value.properties))
        return false;
    return isCubeKind(value.properties.sugarcubes_kind);
}
/** Normalize an optional external string list. */
function readStringArray(value) {
    return Array.isArray(value)
        ? value.filter((item) => typeof item === 'string')
        : [];
}
/** Read one non-empty external string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
