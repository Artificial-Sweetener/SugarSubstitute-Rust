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
 * Own route-based cube identity helpers and model-family metadata normalization.
 */

import { parseCanonicalCubeId, suggestCanonicalCubePath } from './CubeId.js';

/**
 * Target model label for cubes that are intentionally cross-family.
 */
export const ANY_TARGET_MODEL = 'Any';

/**
 * Default target model for new authored cubes.
 */
export const DEFAULT_TARGET_MODEL = 'SDXL';

/**
 * Initial user-facing target model choices for cube authoring.
 */
export const TARGET_MODEL_OPTIONS = Object.freeze([
  'Aura Flow',
  'Anima',
  'Chroma',
  'Flux',
  ANY_TARGET_MODEL,
  'SD 1.5',
  'SDXL',
  'SeedVR2',
  'Wan Video',
]);

const DEFAULT_SUPPORTED_MODELS: Readonly<Record<string, readonly string[]>> = Object.freeze({
  SDXL: Object.freeze(['SDXL', 'SD 1.5']),
});

const MODEL_LABEL_ALIASES: Readonly<Record<string, string>> = Object.freeze({
  'sdxl 1.0': 'SDXL',
});

const WINDOWS_UNSAFE_PATH_SEGMENT_RE = /[<>:"|?*]/;

interface SupportedModelOptions {
  targetModel?: unknown;
}

interface CubeRouteOptions {
  sourceCubeId: unknown;
  route: unknown;
}

interface TargetModelCubeIdOptions {
  sourceCubeId: unknown;
  targetModel: unknown;
  defaultAlias: unknown;
}

function hasAsciiControlCharacter(value: string): boolean {
  for (const char of value) {
    if (char.charCodeAt(0) <= 31) {
      return true;
    }
  }
  return false;
}

/**
 * Return a path-safe target model label or an empty string.
 */
export function normalizeTargetModel(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }
  const normalized = normalizeModelLabel(value);
  if (!normalized) {
    return '';
  }
  validateTargetModelSegment(normalized);
  return normalized;
}

/**
 * Return ordered supported model labels, including the target model when required.
 */
export function normalizeSupportedModels(
  value: unknown,
  { targetModel = '' }: SupportedModelOptions = {},
): string[] {
  const rawItems = typeof value === 'string' ? value.split(',') : Array.isArray(value) ? value : [];
  const models = rawItems
    .filter((entry): entry is string => typeof entry === 'string')
    .map((entry) => normalizeModelLabel(entry))
    .filter(Boolean);
  const normalizedTarget = normalizeTargetModel(targetModel);
  if (normalizedTarget && normalizedTarget !== ANY_TARGET_MODEL) {
    models.unshift(normalizedTarget);
  }
  return dedupePreservingOrder(models);
}

/**
 * Return the default supported model labels for a target model.
 */
export function defaultSupportedModelsForTarget(targetModel: unknown): string[] {
  const normalizedTarget = normalizeTargetModel(targetModel);
  if (DEFAULT_SUPPORTED_MODELS[normalizedTarget]) {
    return [...DEFAULT_SUPPORTED_MODELS[normalizedTarget]];
  }
  return normalizeSupportedModels([], { targetModel: normalizedTarget });
}

/**
 * Return a path-safe cube route without a `.cube` suffix.
 */
export function normalizeCubeRoute(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }
  const normalized = value.trim().replace(/\\/g, '/').replace(/\s+/g, ' ');
  const withoutSuffix = normalized.toLowerCase().endsWith('.cube')
    ? normalized.slice(0, -5)
    : normalized;
  if (!withoutSuffix) {
    return '';
  }
  return withoutSuffix
    .split('/')
    .map((segment) => {
      const cleaned = segment.trim().replace(/\s+/g, ' ');
      validateRouteSegment(cleaned);
      return cleaned;
    })
    .join('/');
}

/**
 * Return the source-relative cube route used as the default alias.
 */
export function deriveRouteFromCubeId(cubeId: unknown): string {
  const parsed = parseCanonicalCubeId(cubeId);
  return normalizeCubeRoute(parsed.path.replace(/\.cube$/i, ''));
}

/**
 * Return the target model segment implied by a cube route.
 */
export function deriveTargetModelFromRoute(route: unknown): string {
  const normalized = normalizeCubeRoute(route);
  if (!normalized.includes('/')) {
    return '';
  }
  return normalizeTargetModel(normalized.split('/')[0]);
}

/**
 * Return the target model segment implied by a canonical cube id route.
 */
export function deriveTargetModelFromCubeId(cubeId: unknown): string {
  return deriveTargetModelFromRoute(deriveRouteFromCubeId(cubeId));
}

/**
 * Return the cube filename implied by a route.
 */
export function deriveFilenameFromRoute(route: unknown): string {
  const normalized = normalizeCubeRoute(route);
  if (!normalized) {
    throw new Error('Cube route is required.');
  }
  return suggestCanonicalCubePath(normalized.split('/').pop() || '');
}

/**
 * Build a canonical cube id with the same source and a route-derived path.
 */
export function deriveCubeIdFromRoute({ sourceCubeId, route }: CubeRouteOptions): string {
  const parsed = parseCanonicalCubeId(sourceCubeId);
  const normalized = normalizeCubeRoute(route);
  if (!normalized) {
    throw new Error('Cube route is required.');
  }
  const segments = normalized.split('/');
  const lastIndex = segments.length - 1;
  segments[lastIndex] = deriveFilenameFromRoute(segments[lastIndex]);
  return `${parsed.sourceRoot}/${segments.join('/')}`;
}

/**
 * Validate that a persisted default alias matches its canonical cube id route.
 */
export function validateCubeRouteIdentity(cubeId: unknown, defaultAlias: unknown): void {
  const expected = deriveRouteFromCubeId(cubeId);
  const actual = normalizeCubeRoute(defaultAlias);
  if (actual !== expected) {
    throw new Error(`Cube default_alias must match cube route '${expected}'.`);
  }
}

/**
 * Build a canonical cube id under the target-model folder.
 */
export function deriveTargetModelCubeId({
  sourceCubeId,
  targetModel,
  defaultAlias,
}: TargetModelCubeIdOptions): string {
  const parsed = parseCanonicalCubeId(sourceCubeId);
  const normalizedTarget = normalizeTargetModel(targetModel);
  if (!normalizedTarget) {
    throw new Error('Cube target model is required.');
  }
  const nameRoute = normalizeCubeRoute(defaultAlias);
  const name = nameRoute ? nameRoute.split('/').pop() : 'cube';
  const filename = suggestCanonicalCubePath(name);
  return `${parsed.sourceRoot}/${normalizedTarget}/${filename}`;
}

function validateRouteSegment(value: string): void {
  if (!value) {
    throw new Error('Cube route must not contain empty segments.');
  }
  if (value === '.' || value === '..') {
    throw new Error('Cube route segment is invalid.');
  }
  if (/[ .]$/.test(value)) {
    throw new Error('Cube route segment must not end with a space or dot.');
  }
  if (WINDOWS_UNSAFE_PATH_SEGMENT_RE.test(value) || hasAsciiControlCharacter(value)) {
    throw new Error('Cube route segment contains invalid characters.');
  }
}

function validateTargetModelSegment(value: string): void {
  if (!value || value === '.' || value === '..') {
    throw new Error('Cube target model is invalid.');
  }
  if (value.includes('/') || value.includes('\\')) {
    throw new Error('Cube target model must be one path segment.');
  }
  if (/[ .]$/.test(value)) {
    throw new Error('Cube target model must not end with a space or dot.');
  }
  if (WINDOWS_UNSAFE_PATH_SEGMENT_RE.test(value) || hasAsciiControlCharacter(value)) {
    throw new Error('Cube target model contains invalid characters.');
  }
}

function normalizeModelLabel(value: string): string {
  const normalized = value.trim().replace(/\s+/g, ' ');
  return MODEL_LABEL_ALIASES[normalized.toLowerCase()] || normalized;
}

function dedupePreservingOrder(values: readonly string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    const key = value.toLowerCase();
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(value);
  }
  return result;
}
