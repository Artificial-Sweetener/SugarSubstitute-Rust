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
/** Derive collision-safe canonical identities for first-time Cube persistence. */

import { normalizeDefaultAliasTitle, suggestCanonicalCubePath } from '../core/CubeId.js';
import { normalizeTargetModel } from '../core/ModelTargets.js';
import type { PersonalCubeIdentity } from './PersonalCubeIdentity.js';

export type CubeSaveDestination =
  | { kind: 'local' }
  | { kind: 'pack'; owner: string; repo: string; repoRef: string };

/** Derive one canonical Cube identity in the selected writable destination. */
export function suggestCubeAuthoringIdentity(
  name: unknown,
  targetModel: unknown,
  destination: CubeSaveDestination,
  existingCubeIds: readonly unknown[] = [],
): PersonalCubeIdentity {
  const requestedName = normalizeDefaultAliasTitle(name) || 'SugarCube';
  const normalizedTargetModel = normalizeTargetModel(targetModel);
  if (!normalizedTargetModel) throw new Error('Target model is required.');
  const usedIds = new Set(
    existingCubeIds
      .filter((cubeId): cubeId is string => typeof cubeId === 'string')
      .map((cubeId) => cubeId.trim().toLowerCase())
      .filter(Boolean),
  );
  let resolvedName = requestedName;
  let suffix = 2;
  while (usedIds.has(buildCubeId(resolvedName, normalizedTargetModel, destination).toLowerCase())) {
    resolvedName = `${requestedName} ${suffix}`;
    suffix += 1;
  }
  return {
    name: resolvedName,
    defaultAlias: `${normalizedTargetModel}/${resolvedName}`,
    cubeId: buildCubeId(resolvedName, normalizedTargetModel, destination),
  };
}

/** Build the canonical source-qualified id understood by the existing save contract. */
function buildCubeId(name: string, targetModel: string, destination: CubeSaveDestination): string {
  const path = `${targetModel}/${suggestCanonicalCubePath(name)}`;
  return destination.kind === 'local'
    ? `local/personal/${path}`
    : `${destination.owner}/${destination.repo}/${path}`;
}
