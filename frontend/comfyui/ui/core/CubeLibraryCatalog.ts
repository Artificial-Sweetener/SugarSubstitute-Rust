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
/** Own validated library metadata independently of discovery and presentation. */
import type { ApiJsonResult } from './CubeLibraryApi.js';
import { normalizeSupportedModels, normalizeTargetModel } from './ModelTargets.js';
import { isRecord, type UnknownRecord } from '../types/common.js';

export interface CubeLibraryEntry extends UnknownRecord {
  cube_id: string;
  default_alias?: string;
  name?: string;
  version?: string;
  author?: string;
  author_url?: string;
  target_model?: string;
  supported_models?: string[];
  tags?: string[];
  is_writable?: boolean;
  display_name?: string;
  description?: string;
  namespace?: string;
  owner?: string;
  repo?: string;
  relative_path?: string;
  write_block_reason?: string;
  source?: UnknownRecord;
  metadata?: UnknownRecord;
  lineage?: UnknownRecord | null;
}

interface CatalogOptions {
  api: { list(options?: RequestInit): Promise<ApiJsonResult> };
  logger: Pick<Console, 'warn'>;
  onUpdated(entries: readonly CubeLibraryEntry[]): void;
}

/** Publish complete metadata snapshots while retaining the last successful catalog. */
export class CubeLibraryCatalog {
  readonly #options: CatalogOptions;
  #entries: CubeLibraryEntry[] = [];
  #index = new Map<string, CubeLibraryEntry>();
  #loaded = false;
  #refreshing: Promise<CubeLibraryEntry[]> | null = null;
  #controller: AbortController | null = null;

  /** Bind the metadata transport and its surviving consumers. */
  constructor(options: CatalogOptions) {
    this.#options = options;
  }

  /** Return an isolated list without exposing the catalog's array ownership. */
  getCubes(): CubeLibraryEntry[] {
    return this.#entries.slice();
  }

  /** Resolve metadata by the stable Cube identity. */
  getCubeById(cubeId: string): CubeLibraryEntry | null {
    return this.#index.get(cubeId.trim()) ?? null;
  }

  /** Coalesce metadata requests and reuse successfully loaded snapshots. */
  refresh(options: { force?: boolean } = {}): Promise<CubeLibraryEntry[]> {
    if (this.#refreshing) return this.#refreshing;
    if (this.#loaded && !options.force) return Promise.resolve(this.getCubes());
    const operation = this.#load().finally(() => {
      if (this.#refreshing === operation) this.#refreshing = null;
    });
    this.#refreshing = operation;
    return operation;
  }

  /** Cancel pending transport when the owning host is disposed. */
  dispose(): void {
    this.#controller?.abort();
  }

  /** Validate and publish one network result atomically. */
  async #load(): Promise<CubeLibraryEntry[]> {
    const controller = new AbortController();
    this.#controller = controller;
    const timeout = setTimeout(() => controller.abort(), 15000);
    try {
      const { response, data } = await this.#options.api.list({ signal: controller.signal });
      if (!response.ok || data.error || !Array.isArray(data.cubes)) {
        throw new Error('Cube library metadata response is unavailable or incompatible.');
      }
      const entries = data.cubes
        .map(readCubeLibraryEntry)
        .filter((entry): entry is CubeLibraryEntry => entry !== null);
      const index = new Map(entries.map((entry) => [entry.cube_id, entry]));
      if (index.size !== entries.length)
        throw new Error('Cube library metadata contains duplicate identities.');
      if (controller.signal.aborted) return this.getCubes();
      this.#entries = entries;
      this.#index = index;
      this.#loaded = true;
      this.#options.onUpdated(this.getCubes());
    } catch (error: unknown) {
      this.#options.logger.warn(
        'SugarCubes library metadata refresh failed; retaining the previous catalog.',
        { error },
      );
    } finally {
      clearTimeout(timeout);
      if (this.#controller === controller) this.#controller = null;
    }
    return this.getCubes();
  }
}

/** Narrow dynamic metadata while preserving extension fields and fork provenance. */
function readCubeLibraryEntry(value: unknown): CubeLibraryEntry | null {
  if (!isRecord(value) || typeof value.cube_id !== 'string' || !value.cube_id.trim()) return null;
  const targetModel = normalizeTargetModel(value.target_model);
  const entry: CubeLibraryEntry = {
    ...value,
    cube_id: value.cube_id.trim(),
    target_model: targetModel,
    supported_models: normalizeSupportedModels(value.supported_models, { targetModel }),
  };
  for (const key of [
    'default_alias',
    'name',
    'version',
    'author',
    'author_url',
    'display_name',
    'description',
    'namespace',
    'owner',
    'repo',
    'relative_path',
    'write_block_reason',
  ] as const) {
    if (typeof entry[key] !== 'string') Reflect.deleteProperty(entry, key);
  }
  if (typeof entry.is_writable !== 'boolean') delete entry.is_writable;
  if (Array.isArray(value.tags))
    entry.tags = value.tags.filter((tag): tag is string => typeof tag === 'string');
  else delete entry.tags;
  for (const key of ['source', 'metadata'] as const) {
    if (!isRecord(entry[key])) Reflect.deleteProperty(entry, key);
  }
  if (entry.lineage !== null && !isRecord(entry.lineage)) delete entry.lineage;
  return entry;
}
