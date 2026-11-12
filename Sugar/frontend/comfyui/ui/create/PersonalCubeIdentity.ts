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
import { normalizeTargetModel } from '../core/ModelTargets.js';

const PERSONAL_SOURCE_ROOT = 'local/personal';

export interface PersonalCubeIdentity {
  name: string;
  defaultAlias: string;
  cubeId: string;
}

/** Return a collision-safe personal identity scoped to its target-model folder. */
export function suggestPersonalCubeIdentity(
  name: unknown,
  targetModel: unknown,
  existingCubeIds: readonly unknown[] = [],
): PersonalCubeIdentity {
  const requestedName = normalizeDefaultAliasTitle(name) || 'SugarCube';
  const normalizedTargetModel = normalizeTargetModel(targetModel);
  if (!normalizedTargetModel) {
    throw new Error('Target model is required.');
  }
  const usedIds = new Set(
    (Array.isArray(existingCubeIds) ? existingCubeIds : [])
      .filter((cubeId) => typeof cubeId === 'string')
      .map((cubeId) => cubeId.trim().toLowerCase())
      .filter(Boolean),
  );
  let resolvedName = requestedName;
  let suffix = 2;
  while (usedIds.has(buildPersonalCubeId(resolvedName, normalizedTargetModel).toLowerCase())) {
    resolvedName = `${requestedName} ${suffix}`;
    suffix += 1;
  }
  return {
    name: resolvedName,
    defaultAlias: `${normalizedTargetModel}/${resolvedName}`,
    cubeId: buildPersonalCubeId(resolvedName, normalizedTargetModel),
  };
}

/** Build one canonical personal cube id under its target-model folder. */
export function buildPersonalCubeId(name: unknown, targetModel: unknown): string {
  const normalizedTargetModel = normalizeTargetModel(targetModel);
  if (!normalizedTargetModel) {
    throw new Error('Target model is required.');
  }
  return `${PERSONAL_SOURCE_ROOT}/${normalizedTargetModel}/${suggestCanonicalCubePath(name)}`;
}
