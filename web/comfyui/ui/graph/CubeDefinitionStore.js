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

const DEFAULT_TTL_MS = 60000;

/**
 * Coordinate cube definition store behavior for the SugarCubes UI.
 */
export class CubeDefinitionStore {
  constructor({ api, logger, ttlMs, onUpdate } = {}) {
    this.api = api;
    this.logger = logger || null;
    this.ttlMs = Number.isFinite(ttlMs) ? ttlMs : DEFAULT_TTL_MS;
    this.onUpdate = typeof onUpdate === 'function' ? onUpdate : null;
    this.entries = new Map();
  }

  getEntry(request) {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey) || null;
  }

  getHash(request) {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey)?.hash || null;
  }

  getStatus(request) {
    const resolved = resolveDefinitionRequest(request);
    return this.entries.get(resolved.definitionKey)?.status || null;
  }

  /** Remove every cached revision belonging to one retired cube identity. */
  invalidateCube(cubeId) {
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

  ensure(request) {
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
    this.loadDefinition(resolved);
    return this.entries.get(resolved.definitionKey) || null;
  }

  /**
   * Publish the canonical definition returned by a successful save.
   *
   * A save is authoritative even when the same definition key is already
   * cached. Replacing that entry prevents a remake from continuing to use the
   * pre-save definition until the normal cache TTL expires.
   */
  publishFinalized(request, payload) {
    const cube = payload?.cube;
    const resolved = resolveDefinitionRequest({
      ...request,
      cubeId: request?.cubeId || cube?.cube_id,
      cubeVersion: request?.cubeVersion || cube?.version,
    });
    const hash = computeDefinitionHash(payload);
    if (!resolved.cubeId || !resolved.definitionKey || !hash) {
      throw new Error('Finalized cube definition is invalid');
    }
    const now = Date.now();
    const entry = {
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
  evictCurrentAliases(resolved) {
    if (!resolved.cubeVersion || !isCurrentRevisionRef(resolved.revisionRef)) {
      return;
    }
    const unversionedKey = buildCubeDefinitionKey(resolved.cubeId, '');
    if (unversionedKey !== resolved.definitionKey) {
      this.entries.delete(unversionedKey);
    }
  }

  async loadDefinition(request) {
    const resolved = resolveDefinitionRequest(request);
    if (!resolved.cubeId || !resolved.definitionKey) {
      return;
    }
    if (!this.api?.load) {
      this.logger?.warn?.('SugarCubes: definition load unavailable', resolved.cubeId);
      return;
    }
    const current = this.entries.get(resolved.definitionKey);
    if (current?.status === 'loading') {
      return;
    }
    const loading = {
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
      if (!response.ok || data?.error) {
        const message = data?.error?.message || response.statusText || 'Definition load failed';
        const entry = {
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
      const entry = {
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
    } catch (error) {
      const message = error?.message ? String(error.message) : String(error);
      const entry = {
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

function resolveDefinitionRequest(request) {
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
