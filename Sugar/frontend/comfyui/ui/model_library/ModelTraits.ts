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
/** Validate reusable model facts and server-owned compatibility decisions at the catalog boundary. */
import { isRecord } from '../types/common.js';

export interface ModelTraits {
  readonly architecture: string;
  readonly architectureSource: 'comfy_tensor_shapes' | 'civitai_relationship' | 'unknown';
  readonly family: string;
  readonly familyName: string;
  readonly familySource: 'civitai' | 'metadata' | 'unknown';
  readonly modelType: string;
  readonly conflict: boolean;
  readonly relationshipRevision: string;
  readonly nsfw: 'sfw' | 'nsfw' | 'unknown';
  readonly providerNsfw: boolean | null;
  readonly providerNsfwLevel: number | null;
  readonly nsfwOverride: boolean | null;
  readonly compatibleTargets: readonly {
    readonly family: string;
    readonly support: 'full' | 'partial';
  }[];
}

/** Accept historical catalogs while rejecting malformed additive evidence. */
export function decodeModelTraits(value: unknown): ModelTraits | null {
  if (value === undefined || value === null) return null;
  if (
    !isRecord(value) ||
    typeof value.conflict !== 'boolean' ||
    !Array.isArray(value.compatibleTargets)
  )
    throw new Error('Invalid model traits.');
  const architectureSource = value.architectureSource;
  const familySource = value.familySource;
  const nsfw = value.nsfw;
  if (
    (architectureSource !== 'comfy_tensor_shapes' &&
      architectureSource !== 'civitai_relationship' &&
      architectureSource !== 'unknown') ||
    (familySource !== 'civitai' && familySource !== 'metadata' && familySource !== 'unknown') ||
    (nsfw !== 'sfw' && nsfw !== 'nsfw' && nsfw !== 'unknown')
  )
    throw new Error('Invalid model trait provenance.');
  const level = value.providerNsfwLevel;
  if (level !== null && (typeof level !== 'number' || !Number.isSafeInteger(level) || level < 0))
    throw new Error('Invalid model NSFW level.');
  return {
    architecture: text(value.architecture),
    architectureSource,
    family: text(value.family),
    familyName: text(value.familyName),
    familySource,
    modelType: text(value.modelType),
    conflict: value.conflict,
    relationshipRevision: text(value.relationshipRevision),
    nsfw,
    providerNsfw: classification(value.providerNsfw),
    providerNsfwLevel: level,
    nsfwOverride: classification(value.nsfwOverride),
    compatibleTargets: value.compatibleTargets.map((item: unknown) => {
      if (!isRecord(item) || (item.support !== 'full' && item.support !== 'partial'))
        throw new Error('Invalid model compatibility decision.');
      return { family: text(item.family), support: item.support };
    }),
  };
}

/** Narrow explicit booleans without turning missing ratings into SFW evidence. */
function classification(value: unknown): boolean | null {
  if (value === null || typeof value === 'boolean') return value;
  throw new Error('Invalid model NSFW classification.');
}

/** Preserve labels literally for safe presentation and exact matching. */
function text(value: unknown): string {
  if (typeof value !== 'string') throw new Error('Invalid model trait label.');
  return value;
}
