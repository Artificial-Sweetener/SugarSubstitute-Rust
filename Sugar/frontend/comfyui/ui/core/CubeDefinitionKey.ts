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
 * Own version-aware SugarCube definition identity helpers.
 */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

/** Identify the mutable working-tree revision at the host boundary. */
export const CURRENT_REVISION_REF = 'WORKTREE';

/**
 * Normalize one semantic cube version string.
 */
export function normalizeCubeVersion(value: unknown): string {
  const raw = typeof value === 'string' ? value.trim() : '';
  if (!raw) {
    return '';
  }
  return raw.replace(/^v/i, '').trim();
}

/**
 * Format one cube version for compact UI display.
 */
export function formatCubeVersionLabel(value: unknown): string {
  const version = normalizeCubeVersion(value);
  return version ? `v${version}` : '';
}

/**
 * Normalize one revision ref for frontend routing and metadata.
 */
export function normalizeRevisionRef(value: unknown): string {
  const ref = typeof value === 'string' && value.trim() ? value.trim() : CURRENT_REVISION_REF;
  return ref || CURRENT_REVISION_REF;
}

/**
 * Return whether the revision ref points at the current working tree.
 */
export function isCurrentRevisionRef(value: unknown): boolean {
  return normalizeRevisionRef(value) === CURRENT_REVISION_REF;
}

/**
 * Build the frontend definition key used for version-aware caches.
 */
export function buildCubeDefinitionKey(cubeId: unknown, version: unknown): string {
  const id = typeof cubeId === 'string' ? cubeId.trim() : '';
  if (!id) {
    return '';
  }
  const normalizedVersion = normalizeCubeVersion(version);
  return normalizedVersion ? `${id}@${normalizedVersion}` : id;
}

/**
 * Enrich one SugarCubes metadata payload with version-aware identity.
 */
export interface CubeDefinitionIdentityOptions {
  cubeId?: unknown;
  version?: unknown;
  revisionRef?: unknown;
}

/** Apply normalized definition identity fields to one metadata record. */
export function applyCubeDefinitionIdentity<T>(
  metadata: T,
  { cubeId, version, revisionRef }: CubeDefinitionIdentityOptions = {},
): T {
  if (!isRecord(metadata)) {
    return metadata;
  }
  const resolvedCubeId =
    typeof cubeId === 'string' && cubeId.trim()
      ? cubeId.trim()
      : typeof metadata.cube_id === 'string'
        ? metadata.cube_id.trim()
        : '';
  const resolvedVersion =
    normalizeCubeVersion(version) || normalizeCubeVersion(metadata.cube_version);
  const resolvedRevisionRef = normalizeRevisionRef(revisionRef || metadata.cube_revision_ref);
  const identified: UnknownRecord = {
    ...metadata,
    cube_id: resolvedCubeId || metadata.cube_id,
    cube_version: resolvedVersion || metadata.cube_version || '',
    cube_revision_ref: resolvedRevisionRef,
    cube_definition_key: buildCubeDefinitionKey(resolvedCubeId, resolvedVersion),
  };
  return identified as T;
}
