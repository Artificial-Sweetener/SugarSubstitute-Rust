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
/** Own version-history and exact Cube artifact access at the backend boundary. */

import type { ApiJsonResult } from '../../core/CubeLibraryApi.js';
import { isCurrentRevisionRef } from '../../core/CubeDefinitionKey.js';
import { readImportPayload, type ImportPayload } from '../../import/PlacementPayload.js';
import { isRecord } from '../../types/common.js';
import { BudgetedMemoryCache } from '../../core/memory/BudgetedMemoryCache.js';
import type { MemoryBudgetRegistry } from '../../core/memory/MemoryBudgetRegistry.js';
import { projectCubeVersionOptions } from './CubeVersionOptionProjection.js';
import type { CubeRevision, CubeVersionOption } from './CubeVersionTypes.js';

export interface CubeVersionApi {
  listRevisions(cubeId: string): Promise<ApiJsonResult>;
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

/**
 * Retrieve validated version options and exact prepared artifacts.
 *
 * Prepared artifacts are fetched data retained for the page lifetime under the shared memory
 * allowance, so version menus stay instant across workflow switches. Library mutations retire
 * them through {@link invalidate}.
 */
export class CubeVersionRepository {
  readonly #api: CubeVersionApi;
  readonly #logger: Pick<Console, 'warn'>;
  readonly #artifacts: BudgetedMemoryCache<string, Promise<ImportPayload>>;

  /** Bind the typed SugarCubes API boundary and the page memory allowance. */
  constructor(api: CubeVersionApi, memory: MemoryBudgetRegistry, logger: Pick<Console, 'warn'>) {
    this.#api = api;
    this.#logger = logger;
    this.#artifacts = new BudgetedMemoryCache({ id: 'cube-version-artifacts', registry: memory });
  }

  /** Retire prepared artifacts of one Cube, or of every Cube, after a library mutation. */
  invalidate(cubeId?: string): void {
    const normalized = cubeId?.trim();
    if (!normalized) this.#artifacts.clear();
    else this.#artifacts.deleteWhere((key) => key.startsWith(`${normalized}\u0000`));
  }

  /** List unique semantic versions whose exact artifacts pass loader validation. */
  async listOptions(cubeId: string, fallbackVersion: string): Promise<CubeVersionOption[]> {
    const requiredCubeId = requireCubeId(cubeId);
    const { response, data } = await this.#api.listRevisions(requiredCubeId);
    if (!response.ok || data.error)
      throw new Error(readApiError(data) || 'Cube versions unavailable.');
    const revisions = Array.isArray(data.version_revisions)
      ? data.version_revisions.filter(isRecord).map((value) => value as CubeRevision)
      : [];
    const options = projectCubeVersionOptions(revisions, fallbackVersion);
    const available = await Promise.all(
      options.map(async (option): Promise<CubeVersionOption | null> => {
        try {
          await this.#loadCachedArtifact(requiredCubeId, option);
          return option;
        } catch (error: unknown) {
          this.#logger.warn(
            `SugarCubes: omitted unavailable Cube version '${option.label}' for '${requiredCubeId}'.`,
            error,
          );
          return null;
        }
      }),
    );
    return available.filter((option): option is CubeVersionOption => option !== null);
  }

  /** Load the exact artifact represented by one version option. */
  async loadArtifact(cubeId: string, option: CubeVersionOption): Promise<ImportPayload> {
    return this.#loadCachedArtifact(requireCubeId(cubeId), option);
  }

  /** Coalesce loader validation and preserve prepared immutable artifacts for switching. */
  #loadCachedArtifact(cubeId: string, option: CubeVersionOption): Promise<ImportPayload> {
    const key = `${cubeId}\u0000${option.value}\u0000${option.revisionRef}`;
    const existing = this.#artifacts.get(key);
    if (existing) return existing;
    const request = this.#loadArtifact(cubeId, option).then(
      (payload) => {
        // Size the retained artifact once it exists; UTF-16 text approximates its heap cost.
        if (this.#artifacts.has(key))
          this.#artifacts.resize(key, JSON.stringify(payload).length * 2);
        return payload;
      },
      (error: unknown) => {
        if (this.#artifacts.has(key)) this.#artifacts.delete(key);
        throw error;
      },
    );
    this.#artifacts.set(key, request, { tier: 3, bytes: 0 });
    return request;
  }

  /** Retrieve and validate one exact prepared artifact from the backend. */
  async #loadArtifact(cubeId: string, option: CubeVersionOption): Promise<ImportPayload> {
    const body = isCurrentRevisionRef(option.revisionRef)
      ? { cube_id: cubeId, origin: { x: 0, y: 0 } }
      : {
          cube_id: cubeId,
          revision_ref: option.revisionRef,
          version_pin: option.value,
          origin: { x: 0, y: 0 },
        };
    const loader = isCurrentRevisionRef(option.revisionRef)
      ? this.#api.load.bind(this.#api)
      : this.#api.loadRevision.bind(this.#api);
    const { response, data } = await loader(JSON.stringify(body), {
      headers: { 'Content-Type': 'application/json' },
    });
    if (!response.ok || data.error)
      throw new Error(readApiError(data) || 'Cube version failed to load.');
    const payload = readImportPayload(data);
    if (!payload) throw new Error('Cube version returned an invalid artifact.');
    return payload;
  }
}

/** Require one durable Cube identity before crossing the API boundary. */
function requireCubeId(value: string): string {
  const cubeId = value.trim();
  if (!cubeId) throw new TypeError('Cube identity is required for version access.');
  return cubeId;
}

/** Read one actionable backend error without trusting arbitrary response values. */
function readApiError(data: Record<string, unknown>): string {
  if (typeof data.error === 'string') return data.error.trim();
  if (!isRecord(data.error)) return '';
  const message = typeof data.error.message === 'string' ? data.error.message.trim() : '';
  const detail = typeof data.error.detail === 'string' ? data.error.detail.trim() : '';
  return [message, detail].filter(Boolean).join(' ');
}
