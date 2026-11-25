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
import type { ComfyGraph, ComfyHostApi } from '../../types/graph.js';
import type { CubeStackView } from '../chain/CubeStackView.js';
import { CubeExecutionApiClient } from './CubeExecutionApiClient.js';
import type { ComfyPromptQueueOptions } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import type { WorkflowPromptPreparation } from '../../execution/domain/WorkflowPreparation.js';

export interface CubeDirectExecutionServiceOptions {
  prepareInputs?(payload: unknown): Promise<WorkflowPromptPreparation>;
  /** Give the serialized workflow the prompt texts of the scene its queued prompt renders. */
  sceneWorkflow?(payload: unknown, workflow: unknown): unknown;
  /** Resolve what the workflow's prompt texts name for this run, such as wildcards. */
  resolveWorkflow?(payload: unknown, workflow: unknown): Promise<unknown>;
  client: CubeExecutionApiClient;
  hostApi?: ComfyHostApi;
  logger?: Pick<Console, 'debug'>;
  getGraph(): ComfyGraph | null | undefined;
  /** The stack the page shows: its order and proximity links; null without Cubes. */
  getStack(): CubeStackView | null;
}

export interface CreateCubeDirectExecutionOptions {
  prepareInputs?(payload: unknown): Promise<WorkflowPromptPreparation>;
  sceneWorkflow?(payload: unknown, workflow: unknown): unknown;
  /** Resolve what the workflow's prompt texts name for this run, such as wildcards. */
  resolveWorkflow?(payload: unknown, workflow: unknown): Promise<unknown>;
  api: unknown;
  logger?: Pick<Console, 'debug'>;
  getGraph(): ComfyGraph | null | undefined;
  /** The stack the page shows: its order and proximity links; null without Cubes. */
  getStack(): CubeStackView | null;
}

/** Own browser-side request projection without interpreting Cube graph semantics. */
export class CubeDirectExecutionService {
  constructor(private readonly options: CubeDirectExecutionServiceOptions) {}

  /** Return whether a queue payload belongs to native SugarCubes execution. */
  owns(payload: unknown): boolean {
    if (!isRecord(payload) || !isRecord(payload.workflow)) return false;
    if (isRecord(payload.extra_data) && isRecord(payload.extra_data.substitute)) return false;
    const nodes = payload.workflow.nodes;
    return Array.isArray(nodes) && nodes.some(isPersistedCubeNode);
  }

  /** Serialize live workflow state and delegate execution to SugarCubes once. */
  async queue(
    position: number,
    comfyPayload: unknown,
    queueOptions: ComfyPromptQueueOptions = {},
    preparedInput?: (preparation: WorkflowPromptPreparation) => void,
  ): Promise<unknown> {
    const graph = this.options.getGraph();
    if (!graph) throw new TypeError('The active Comfy graph is unavailable.');
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
        partial_execution_targets: readStringArray(
          queueOptions.partialExecutionTargets ?? payload.partial_execution_targets,
        ),
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
export function createCubeDirectExecution(
  options: CreateCubeDirectExecutionOptions,
): CubeDirectExecutionService {
  return new CubeDirectExecutionService({
    client: new CubeExecutionApiClient(options.api as ComfyHostApi),
    hostApi: options.api as ComfyHostApi,
    ...(options.logger ? { logger: options.logger } : {}),
    getGraph: options.getGraph,
    getStack: options.getStack,
    ...(options.prepareInputs ? { prepareInputs: options.prepareInputs } : {}),
    ...(options.sceneWorkflow ? { sceneWorkflow: options.sceneWorkflow } : {}),
    ...(options.resolveWorkflow ? { resolveWorkflow: options.resolveWorkflow } : {}),
  });
}

/** Preserve native Comfy history and sensitive metadata for direct queueing. */
function buildExtraData(
  workflow: unknown,
  payload: Record<string, unknown>,
  queueOptions: ComfyPromptQueueOptions,
  hostApi: ComfyHostApi | undefined,
): Record<string, unknown> {
  const result = isRecord(payload.extra_data) ? { ...payload.extra_data } : {};
  result.comfy_usage_source = 'comfyui-frontend';
  result.extra_pnginfo = { workflow };
  copyNonEmptyString(result, 'auth_token_comfy_org', hostApi?.authToken);
  copyNonEmptyString(result, 'api_key_comfy_org', hostApi?.apiKey);
  const previewMethod = readString(queueOptions.previewMethod);
  if (previewMethod && previewMethod !== 'default') result.preview_method = previewMethod;
  return result;
}

/** Copy one secret-bearing Comfy value only when the host supplied text. */
function copyNonEmptyString(target: Record<string, unknown>, key: string, value: unknown): void {
  const normalized = readString(value);
  if (normalized) target[key] = normalized;
}

/** Identify a durable native Cube marker without resolving its definition. */
function isPersistedCubeNode(value: unknown): boolean {
  if (!isRecord(value) || !isRecord(value.properties)) return false;
  return isCubeKind(value.properties.sugarcubes_kind);
}

/** Normalize an optional external string list. */
function readStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

/** Read one non-empty external string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
