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
 * Derive collision-safe identities for zero-setup personal cube creation.
 */

import { normalizeDefaultAliasTitle, suggestCanonicalCubePath } from '../core/CubeId.js';

const PERSONAL_SOURCE_ROOT = 'local/personal';

export interface PersonalCubeIdentity {
  name: string;
  defaultAlias: string;
  cubeId: string;
}

/** Return a collision-safe personal identity and display name. */
export function suggestPersonalCubeIdentity(
  name: unknown,
  existingCubeIds: readonly unknown[] = [],
): PersonalCubeIdentity {
  const requestedName = normalizeDefaultAliasTitle(name) || 'SugarCube';
  const usedIds = new Set(
    (Array.isArray(existingCubeIds) ? existingCubeIds : [])
      .filter((cubeId) => typeof cubeId === 'string')
      .map((cubeId) => cubeId.trim().toLowerCase())
      .filter(Boolean),
  );
  let resolvedName = requestedName;
  let suffix = 2;
  while (usedIds.has(buildPersonalCubeId(resolvedName).toLowerCase())) {
    resolvedName = `${requestedName} ${suffix}`;
    suffix += 1;
  }
  return {
    name: resolvedName,
    defaultAlias: resolvedName,
    cubeId: buildPersonalCubeId(resolvedName),
  };
}

/** Build one canonical flat personal cube id. */
export function buildPersonalCubeId(name: unknown): string {
  return `${PERSONAL_SOURCE_ROOT}/${suggestCanonicalCubePath(name)}`;
}
