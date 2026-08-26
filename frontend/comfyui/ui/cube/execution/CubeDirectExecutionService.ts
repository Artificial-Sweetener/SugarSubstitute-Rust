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

import { enrichWorkflowPayload } from '../../graph/WorkflowPayloadBuilder.js';
import { isRecord } from '../../types/common.js';
import type { ComfyGraph, ComfyHostApi } from '../../types/graph.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { requireCubeIdentity } from '../node/ComfyCubeNodeFactory.js';
import { CubeExecutionApiClient } from './CubeExecutionApiClient.js';
import type { ComfyPromptQueueOptions } from './ComfyPromptQueueBridge.js';

export interface CubeDirectExecutionServiceOptions {
  client: CubeExecutionApiClient;
  hostApi?: ComfyHostApi;
  logger?: Pick<Console, 'debug'>;
  getGraph(): ComfyGraph | null | undefined;
  getProximityMatches(): readonly unknown[];
}

export interface CreateCubeDirectExecutionOptions {
  api: unknown;
  logger?: Pick<Console, 'debug'>;
  getGraph(): ComfyGraph | null | undefined;
  getProximityMatches(): readonly unknown[];
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
  ): Promise<unknown> {
    const graph = this.options.getGraph();
    if (!graph) throw new TypeError('The active Comfy graph is unavailable.');
    const serialize = graph.serialize;
    if (typeof serialize !== 'function') {
      throw new TypeError('The active Comfy graph cannot be serialized.');
    }
    const proximityMatches = this.options.getProximityMatches();
    const proximityConnections = proximityMatches.flatMap(toConnection);
    const workflow = enrichWorkflowPayload(serialize.call(graph), graph);
    const payload = isRecord(comfyPayload) ? comfyPayload : {};
    const extraData = buildExtraData(workflow, payload, queueOptions, this.options.hostApi);
    const clientId = readString(this.options.hostApi?.clientId) || readString(payload.client_id);
    const explicitNumber = position !== 0 && position !== -1 ? position : null;
    this.options.logger?.debug('SugarCubes projected direct-execution proximity.', {
      acceptedMatches: proximityMatches.length,
      projectedConnections: proximityConnections.length,
      endpoints: proximityMatches.map(describeMatchProjection),
    });
    return await this.options.client.queue({
      schema_version: 1,
      workflow,
      proximity_connections: proximityConnections,
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
    getProximityMatches: options.getProximityMatches,
  });
}

/** Describe non-sensitive endpoint shape for actionable projection diagnostics. */
function describeMatchProjection(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) return { match: typeof value };
  return {
    output_id: value.outputId,
    input_id: value.inputId,
    output_node_is_cube: isCubeLike(value.outputNode),
    input_node_is_cube: isCubeLike(value.inputNode),
    output_cube_kind: typeof value.outputCube,
    input_cube_kind: typeof value.inputCube,
  };
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
  const kind = value.properties.sugarcubes_kind;
  return kind === 'cube' || kind === 'cube_draft';
}

/** Project only accepted Cube-to-Cube proximity matches into stable boundary identities. */
function toConnection(value: unknown): unknown[] {
  if (!isRecord(value)) return [];
  const match = value;
  const stableConnection = readStableMatchConnection(match);
  if (stableConnection) return [stableConnection];
  const outputCube = isCubeLike(match.outputNode) ? match.outputNode : match.outputCube;
  const inputCube = isCubeLike(match.inputNode) ? match.inputNode : match.inputCube;
  if (!isCubeLike(outputCube) || !isCubeLike(inputCube)) return [];
  const sourceIdentity = requireCubeIdentity(outputCube);
  const targetIdentity = requireCubeIdentity(inputCube);
  const sourceInstanceId = readString(sourceIdentity.instance_id);
  const targetInstanceId = readString(targetIdentity.instance_id);
  const sourceBinding = readBoundaryName(outputCube.subgraph.outputs, match.outputSlot);
  const targetBinding = readBoundaryName(inputCube.subgraph.inputs, match.inputSlot);
  if (!sourceInstanceId || !targetInstanceId || !sourceBinding || !targetBinding) return [];
  return [
    {
      source_instance_id: sourceInstanceId,
      source_binding: sourceBinding,
      target_instance_id: targetInstanceId,
      target_binding: targetBinding,
    },
  ];
}

/** Project matcher-owned endpoint identity without inspecting host node shape. */
function readStableMatchConnection(match: Record<string, unknown>): Record<string, string> | null {
  if (match.outputCube == null || match.inputCube == null) return null;
  const sourceInstanceId = readString(match.outputInstanceId);
  const sourceBinding = readString(match.outputBinding);
  const targetInstanceId = readString(match.inputInstanceId);
  const targetBinding = readString(match.inputBinding);
  if (!sourceInstanceId || !sourceBinding || !targetInstanceId || !targetBinding) return null;
  return {
    source_instance_id: sourceInstanceId,
    source_binding: sourceBinding,
    target_instance_id: targetInstanceId,
    target_binding: targetBinding,
  };
}

/** Narrow a native Cube surface without coupling to its renderer. */
function isCubeLike(value: unknown): value is CubeNode {
  return (
    isRecord(value) &&
    isRecord(value.properties) &&
    isRecord(value.subgraph) &&
    Array.isArray(value.subgraph.inputs) &&
    Array.isArray(value.subgraph.outputs)
  );
}

/** Read one ordered native boundary name. */
function readBoundaryName(entries: unknown[], slotValue: unknown): string {
  if (typeof slotValue !== 'number' || !Number.isInteger(slotValue)) return '';
  const slot = slotValue;
  if (slot < 0 || slot >= entries.length) return '';
  const entry = entries[slot];
  return isRecord(entry) ? readString(entry.name) : '';
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
