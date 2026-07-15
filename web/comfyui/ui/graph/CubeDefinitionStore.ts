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
/**
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/CubeDefinitionStore.js`.
 */

import { computeDefinitionHash } from './DirtyHasher.js';
import {
  buildCubeDefinitionKey,
  isCurrentRevisionRef,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { UnknownRecord } from '../types/common.js';

const DEFAULT_TTL_MS = 60000;

export type CubeDefinitionStatus = 'loading' | 'ready' | 'error';

export interface CubeDefinitionRequest {
  cubeId?: unknown;
  cubeVersion?: unknown;
  revisionRef?: unknown;
  definitionKey?: unknown;
}

export interface ResolvedCubeDefinitionRequest {
  cubeId: string;
  cubeVersion: string;
  revisionRef: string;
  definitionKey: string;
}

export interface CubeDefinitionEntry extends ResolvedCubeDefinitionRequest {
  status: CubeDefinitionStatus;
  hash: string | null;
  payload: UnknownRecord | null;
  error: string | null;
  updatedAt: number;
  expiresAt: number;
}

export interface ReadyCubeDefinitionEntry<T extends UnknownRecord = UnknownRecord>
  extends Omit<CubeDefinitionEntry, 'status' | 'hash' | 'payload' | 'error'> {
  status: 'ready';
  hash: string;
  payload: T;
  error: null;
}

interface DefinitionApi {
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

interface DefinitionLogger {
  warn?(...values: unknown[]): void;
}

interface DefinitionStoreOptions {
  api?: DefinitionApi | null;
  logger?: DefinitionLogger | null;
  ttlMs?: number;
  onUpdate?: ((definitionKey: string, entry: CubeDefinitionEntry) => void) | null;
}

type DefinitionRequestInput = string | CubeDefinitionRequest | null | undefined;

/**
 * Coordinate cube definition store behavior for the SugarCubes UI.
 */
export class CubeDefinitionStore {
  private readonly api: DefinitionApi | null;
  private readonly logger: DefinitionLogger | null;
  private readonly ttlMs: number;
  private readonly onUpdate: ((definitionKey: string, entry: CubeDefinitionEntry) => void) | null;
  private readonly entries: Map<string, CubeDefinitionEntry>;

  constructor({ api = null, logger = null, ttlMs, onUpdate = null }: DefinitionStoreOptions = {}) {
    this.api = api;
    this.logger = logger;
    this.ttlMs = typeof ttlMs === 'number' && Number.isFinite(ttlMs) ? ttlMs : DEFAULT_TTL_MS;
    this.onUpdate = onUpdate;
    this.entries = new Map<string, CubeDefinitionEntry>();
  }

  getEntry(request: DefinitionRequestInput): CubeDefinitionEntry | null {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey) || null;
  }

  getHash(request: DefinitionRequestInput): string | null {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey)?.hash || null;
  }

  getStatus(request: DefinitionRequestInput): CubeDefinitionStatus | null {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey)?.status || null;
  }

  /** Remove every cached revision belonging to one retired cube identity. */
  invalidateCube(cubeId: unknown): number {
    const normalized = typeof cubeId === 'string' ? cubeId.trim() : '';
    if (!normalized) {
      return 0;
    }
    let removed = 0;
    for (const [definitionKey, entry] of this.entries) {
      if (entry?.cubeId !== normalized) {
        continue;
      }
      this.entries.delete(definitionKey);
      removed += 1;
    }
    return removed;
  }

  ensure(request: DefinitionRequestInput): CubeDefinitionEntry | null {
    const resolved = resolveDefinitionRequest(request);
    if (!resolved.cubeId || !resolved.definitionKey) {
      return null;
    }
    const now = Date.now();
    const existing = this.entries.get(resolved.definitionKey);
    if (existing) {
      if (existing.status === 'loading') {
        return existing;
      }
      if (existing.expiresAt && existing.expiresAt > now) {
        return existing;
      }
    }
    void this.loadDefinition(resolved);
    return this.entries.get(resolved.definitionKey) || null;
  }

  /**
   * Publish the canonical definition returned by a successful save.
   *
   * A save is authoritative even when the same definition key is already
   * cached. Replacing that entry prevents a remake from continuing to use the
   * pre-save definition until the normal cache TTL expires.
   */
  publishFinalized<T extends UnknownRecord>(
    request: CubeDefinitionRequest,
    payload: T,
  ): ReadyCubeDefinitionEntry<T> {
    const cube = isRecord(payload.cube) ? payload.cube : {};
    const resolved = resolveDefinitionRequest({
      ...request,
      cubeId: request.cubeId || cube.cube_id,
      cubeVersion: request.cubeVersion || cube.version,
    });
    const hash = computeDefinitionHash(payload);
    if (!resolved.cubeId || !resolved.definitionKey || !hash) {
      throw new Error('Finalized cube definition is invalid');
    }
    const now = Date.now();
    const entry: ReadyCubeDefinitionEntry<T> = {
      status: 'ready',
      hash,
      payload,
      error: null,
      ...resolved,
      updatedAt: now,
      expiresAt: now + this.ttlMs,
    };
    this.entries.set(resolved.definitionKey, entry);
    this.evictCurrentAliases(resolved);
    this.onUpdate?.(resolved.definitionKey, entry);
    return entry;
  }

  /** Remove stale unversioned aliases after publishing a versioned worktree definition. */
  evictCurrentAliases(resolved: ResolvedCubeDefinitionRequest): void {
    if (!resolved.cubeVersion || !isCurrentRevisionRef(resolved.revisionRef)) {
      return;
    }
    const unversionedKey = buildCubeDefinitionKey(resolved.cubeId, '');
    if (unversionedKey !== resolved.definitionKey) {
      this.entries.delete(unversionedKey);
    }
  }

  async loadDefinition(request: DefinitionRequestInput): Promise<void> {
    const resolved = resolveDefinitionRequest(request);
    if (!resolved.cubeId || !resolved.definitionKey) {
      return;
    }
    if (!this.api) {
      this.logger?.warn?.('SugarCubes: definition load unavailable', resolved.cubeId);
      return;
    }
    const current = this.entries.get(resolved.definitionKey);
    if (current?.status === 'loading') {
      return;
    }
    const loading: CubeDefinitionEntry = {
      status: 'loading',
      hash: null,
      payload: null,
      error: null,
      ...resolved,
      updatedAt: Date.now(),
      expiresAt: Date.now() + this.ttlMs,
    };
    this.entries.set(resolved.definitionKey, loading);
    this.onUpdate?.(resolved.definitionKey, loading);
    try {
      const loader = isCurrentRevisionRef(resolved.revisionRef)
        ? this.api.load.bind(this.api)
        : this.api.loadRevision.bind(this.api);
      const body = isCurrentRevisionRef(resolved.revisionRef)
        ? { cube_id: resolved.cubeId, origin: { x: 0, y: 0 } }
        : {
            cube_id: resolved.cubeId,
            revision_ref: resolved.revisionRef,
            version_pin: resolved.cubeVersion || undefined,
            origin: { x: 0, y: 0 },
          };
      const { response, data } = await loader(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
      });
      if (!response.ok || data.error) {
        const message = readApiError(data) || response.statusText || 'Definition load failed';
        const entry: CubeDefinitionEntry = {
          status: 'error',
          hash: null,
          payload: null,
          error: message,
          ...resolved,
          updatedAt: Date.now(),
          expiresAt: Date.now() + this.ttlMs,
        };
        this.entries.set(resolved.definitionKey, entry);
        this.logger?.warn?.('SugarCubes: definition load failed', resolved.cubeId, message);
        this.onUpdate?.(resolved.definitionKey, entry);
        return;
      }
      const hash = computeDefinitionHash(data);
      const status = hash ? 'ready' : 'error';
      const entry: CubeDefinitionEntry = {
        status,
        hash,
        payload: data,
        error: hash ? null : 'Definition hash unavailable',
        ...resolved,
        updatedAt: Date.now(),
        expiresAt: Date.now() + this.ttlMs,
      };
      this.entries.set(resolved.definitionKey, entry);
      this.onUpdate?.(resolved.definitionKey, entry);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      const entry: CubeDefinitionEntry = {
        status: 'error',
        hash: null,
        payload: null,
        error: message,
        ...resolved,
        updatedAt: Date.now(),
        expiresAt: Date.now() + this.ttlMs,
      };
      this.entries.set(resolved.definitionKey, entry);
      this.logger?.warn?.('SugarCubes: definition load failed', resolved.cubeId, message);
      this.onUpdate?.(resolved.definitionKey, entry);
    }
  }
}

/** Normalize a definition lookup into its version-aware cache identity. */
export function resolveDefinitionRequest(
  request: DefinitionRequestInput,
): ResolvedCubeDefinitionRequest {
  if (typeof request === 'string') {
    const cubeId = request.trim();
    return {
      cubeId,
      cubeVersion: '',
      revisionRef: 'WORKTREE',
      definitionKey: cubeId,
    };
  }
  const cubeId = typeof request?.cubeId === 'string' ? request.cubeId.trim() : '';
  const cubeVersion = normalizeCubeVersion(request?.cubeVersion);
  const revisionRef = normalizeRevisionRef(request?.revisionRef);
  const definitionKey =
    typeof request?.definitionKey === 'string' && request.definitionKey.trim()
      ? request.definitionKey.trim()
      : buildCubeDefinitionKey(cubeId, cubeVersion);
  return {
    cubeId,
    cubeVersion,
    revisionRef,
    definitionKey,
  };
}

function readApiError(data: UnknownRecord): string {
  if (typeof data.error === 'string') {
    return data.error;
  }
  return isRecord(data.error) && typeof data.error.message === 'string' ? data.error.message : '';
}
