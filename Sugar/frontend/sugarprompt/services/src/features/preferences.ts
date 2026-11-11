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
/** Validate normalized preference snapshots; persisted schema migration belongs to the host. */
import type { FeatureId, FeaturePreferences, PreferencePatch } from './contracts.js';
import { featureDefinitions, isFeatureId } from './registry.js';

/** Immutable current defaults preserve donor feature, dwell and generation decisions. */
export const defaultPreferences: FeaturePreferences = Object.freeze({
  allowed: Object.freeze(
    Object.fromEntries(featureDefinitions.map(({ id }) => [id, true])) as Record<
      FeatureId,
      boolean
    >,
  ),
  wheelMode: 'hover_dwell',
  resolveOnGeneration: true,
});

/** Require a plain object with only the declared decoded preference keys. */
function record(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).every((key) => keys.includes(key))
  );
}

/** Validate partial data without truthiness coercion or writing over invalid persisted values. */
export function validatePreferencePatch(value: unknown): PreferencePatch | null {
  if (!record(value, ['allowed', 'wheelMode', 'resolveOnGeneration'])) return null;
  if (
    'wheelMode' in value &&
    value.wheelMode !== 'hover_dwell' &&
    value.wheelMode !== 'focus_required'
  )
    return null;
  if ('resolveOnGeneration' in value && typeof value.resolveOnGeneration !== 'boolean') return null;
  if ('allowed' in value) {
    if (
      !record(
        value.allowed,
        featureDefinitions.map(({ id }) => id),
      )
    )
      return null;
    if (
      !Object.entries(value.allowed).every(
        ([id, allowed]) => isFeatureId(id) && typeof allowed === 'boolean',
      )
    )
      return null;
  }
  return value as PreferencePatch;
}

/** Build one frozen snapshot, filling missing new features from registry defaults. */
export function applyPreferencePatch(
  current: FeaturePreferences,
  patch: PreferencePatch,
): FeaturePreferences {
  const validated = validatePreferencePatch(patch);
  if (!validated) throw new TypeError('Invalid preference patch');
  return Object.freeze({
    allowed: Object.freeze({ ...current.allowed, ...validated.allowed }),
    wheelMode: validated.wheelMode ?? current.wheelMode,
    resolveOnGeneration: validated.resolveOnGeneration ?? current.resolveOnGeneration,
  });
}

/** Adapt decoded host data; first use and partial older snapshots receive current defaults. */
export function readPreferences(value: unknown): FeaturePreferences | null {
  if (value === null) return defaultPreferences;
  const patch = validatePreferencePatch(value);
  return patch ? applyPreferencePatch(defaultPreferences, patch) : null;
}
