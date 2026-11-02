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
/** Own non-persisted library classification for the active workflow. */

import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import { isRecord } from '../types/common.js';
import { contentDigest } from '../core/ContentDigest.js';

export type WorkflowCubeLibraryClass = 'none' | 'captured' | 'local' | 'synced';
export type WorkflowCubeAccess = 'read_only' | 'writable';

export interface WorkflowCubeClassification {
  definitionId: string;
  cubeId: string;
  cubeVersion: string;
  semanticHash: string;
  instanceIds: readonly string[];
  primaryClass: WorkflowCubeLibraryClass;
  access: WorkflowCubeAccess;
  sourceAvailable: boolean;
  permittedOperations: ReadonlySet<string>;
}

export interface WorkflowLibraryApi {
  classifyWorkflow(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

interface WorkflowLibraryLogger {
  warn(...values: unknown[]): void;
}

interface ClassificationResponse {
  readonly catalogRevision: string | null;
  readonly definitions: readonly WorkflowCubeClassification[];
}

const CLASSIFICATION_CACHE_CAPACITY = 16;

/**
 * Keep machine-derived classification out of serialized workflow properties.
 *
 * Classification depends only on the embedded Cube definitions, their instances and the installed
 * catalog. Results are retained per classification input under the catalog revision the server
 * reported, so a workflow switch with unchanged definitions issues no request. A response with a
 * different revision, or a local catalog mutation, retires every retained result.
 */
export class CubeWorkflowLibraryState {
  readonly #api: WorkflowLibraryApi;
  readonly #logger: WorkflowLibraryLogger;
  readonly #listeners = new Set<() => void>();
  readonly #results = new Map<string, readonly WorkflowCubeClassification[]>();
  #catalogRevision: string | null = null;
  #catalogEpoch = 0;
  #byInstanceId = new Map<string, WorkflowCubeClassification>();
  #generation = 0;

  /** Bind the versioned SugarCubes API and a host diagnostic sink. */
  constructor(api: WorkflowLibraryApi, logger: WorkflowLibraryLogger) {
    this.#api = api;
    this.#logger = logger;
  }

  /** Start best-effort enrichment without blocking Comfy graph hydration. */
  begin(workflow: unknown): void {
    const generation = ++this.#generation;
    const key = classificationKey(workflow);
    const retained = key === null ? undefined : this.#results.get(key);
    if (key !== null && retained) {
      this.#results.delete(key);
      this.#results.set(key, retained);
      this.#replace(retained);
      return;
    }
    this.#replace([]);
    void this.#classify(workflow, key, generation).catch((error: unknown) => {
      if (generation !== this.#generation) return;
      this.#logger.warn('SugarCubes: workflow Cube classification unavailable', error);
    });
  }

  /** Retire retained classifications after a local catalog mutation. */
  invalidateCatalog(): void {
    this.#catalogEpoch += 1;
    this.#catalogRevision = null;
    this.#results.clear();
  }

  /** Read current machine state for one stable instance identity. */
  read(instanceId: string): WorkflowCubeClassification | null {
    return this.#byInstanceId.get(instanceId) ?? null;
  }

  /** Return whether definition editing is currently authorized for one instance. */
  canEdit(instanceId: string): boolean | null {
    const classification = this.read(instanceId);
    return classification ? classification.access === 'writable' : null;
  }

  /** Observe classification replacement for badges and workflow actions. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Cancel stale requests and clear all machine-derived state. */
  clear(): void {
    this.#generation += 1;
    this.#replace([]);
  }

  /** Submit an immutable workflow snapshot and apply only the latest response. */
  async #classify(workflow: unknown, key: string | null, generation: number): Promise<void> {
    const epoch = this.#catalogEpoch;
    const result = await this.#api.classifyWorkflow(JSON.stringify({ workflow }), {
      headers: { 'Content-Type': 'application/json' },
    });
    if (!result.response.ok) {
      throw new Error(`Workflow Cube classification failed (${String(result.response.status)}).`);
    }
    const { catalogRevision, definitions } = readClassificationResponse(result.data);
    // A local catalog mutation during the request makes its answer unsafe to reuse.
    if (epoch === this.#catalogEpoch) this.#retain(key, catalogRevision, definitions);
    if (generation === this.#generation) this.#replace(definitions);
  }

  /** Retain one result only while its catalog revision remains the latest one observed. */
  #retain(
    key: string | null,
    catalogRevision: string | null,
    definitions: readonly WorkflowCubeClassification[],
  ): void {
    if (catalogRevision !== this.#catalogRevision) {
      this.#results.clear();
      this.#catalogRevision = catalogRevision;
    }
    if (key === null || catalogRevision === null) return;
    this.#results.set(key, definitions);
    while (this.#results.size > CLASSIFICATION_CACHE_CAPACITY) {
      const oldest = this.#results.keys().next().value;
      if (oldest === undefined) break;
      this.#results.delete(oldest);
    }
  }

  /** Atomically replace the instance index and notify presentation subscribers. */
  #replace(definitions: readonly WorkflowCubeClassification[]): void {
    const next = new Map<string, WorkflowCubeClassification>();
    for (const definition of definitions) {
      for (const instanceId of definition.instanceIds) next.set(instanceId, definition);
    }
    this.#byInstanceId = next;
    for (const listener of this.#listeners) listener();
  }
}

/** Validate the versioned response before it becomes active UI state. */
function readClassificationResponse(value: unknown): ClassificationResponse {
  if (!isRecord(value) || value.schema_version !== 1 || !Array.isArray(value.definitions)) {
    throw new TypeError('Workflow Cube classification response is invalid.');
  }
  const revision = typeof value.catalog_revision === 'string' ? value.catalog_revision.trim() : '';
  return {
    catalogRevision: revision || null,
    definitions: value.definitions.map((entry, index) => readClassification(entry, index)),
  };
}

/**
 * Identify exactly the inputs classification reads: embedded definitions and root Cube instances.
 *
 * Node geometry, values and canvas state are excluded so ordinary edits and view changes keep
 * reusing the result; any change to a definition or an instance identity changes the key.
 */
function classificationKey(workflow: unknown): string | null {
  if (!isRecord(workflow) || !Array.isArray(workflow.nodes)) return null;
  const definitions = isRecord(workflow.definitions) ? workflow.definitions.subgraphs : undefined;
  const instances = workflow.nodes.map((node: unknown) => {
    if (!isRecord(node)) return null;
    const properties = isRecord(node.properties) ? node.properties : {};
    return [node.id, node.type, properties.sugarcubes_kind, properties.sugarcubes_cube];
  });
  try {
    return contentDigest(JSON.stringify([definitions ?? null, instances]));
  } catch {
    // Unserializable host values cannot be keyed; classification still runs uncached.
    return null;
  }
}

/** Parse one complete definition classification from an untrusted response. */
function readClassification(value: unknown, index: number): WorkflowCubeClassification {
  if (!isRecord(value))
    throw new TypeError(`Workflow Cube classification #${String(index)} is invalid.`);
  const primaryClass = requireEnum(
    value.primary_class,
    ['none', 'captured', 'local', 'synced'] as const,
    'primary_class',
  );
  const access = requireEnum(value.access, ['read_only', 'writable'] as const, 'access');
  if (
    !Array.isArray(value.instance_ids) ||
    value.instance_ids.some((item) => typeof item !== 'string')
  ) {
    throw new TypeError('Workflow Cube classification instance_ids must be strings.');
  }
  if (
    !Array.isArray(value.permitted_operations) ||
    value.permitted_operations.some((item) => typeof item !== 'string')
  ) {
    throw new TypeError('Workflow Cube classification operations must be strings.');
  }
  return {
    definitionId: requireString(value.definition_id, 'definition_id'),
    cubeId: requireString(value.cube_id, 'cube_id'),
    cubeVersion: requireString(value.cube_version, 'cube_version'),
    semanticHash: requireString(value.semantic_hash, 'semantic_hash'),
    instanceIds: value.instance_ids,
    primaryClass,
    access,
    sourceAvailable: value.source_available === true,
    permittedOperations: new Set(value.permitted_operations),
  };
}

/** Require one non-empty API string. */
function requireString(value: unknown, field: string): string {
  const normalized = typeof value === 'string' ? value.trim() : '';
  if (!normalized) throw new TypeError(`Workflow Cube classification ${field} is required.`);
  return normalized;
}

/** Narrow one response string to an explicit protocol enum. */
function requireEnum<const T extends readonly string[]>(
  value: unknown,
  allowed: T,
  field: string,
): T[number] {
  if (typeof value === 'string' && allowed.includes(value)) return value;
  throw new TypeError(`Workflow Cube classification ${field} is invalid.`);
}
