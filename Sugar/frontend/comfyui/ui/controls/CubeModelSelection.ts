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
/** Own the shared vocabulary and custom-entry policy for Cube model metadata controls. */

import { TARGET_MODEL_OPTIONS } from '../core/ModelTargets.js';
import type { ComfySettingsSelectOption } from './ComfySettingsSelect.js';

/** Represent the non-persisted select action that reveals custom target entry. */
export const CUSTOM_TARGET_MODEL_VALUE = '__sugarcubes_custom_target_model__';

/** Name the explicit escape hatch consistently across Cube authoring surfaces. */
export const ANOTHER_MODEL_OPTION_LABEL = 'Another model…';

/** Present curated and discovered model names before the explicit custom-entry action. */
export function targetModelSelectOptions(
  suggestions: readonly string[] = [],
): ComfySettingsSelectOption[] {
  return [
    ...uniqueModelNames([...TARGET_MODEL_OPTIONS, ...suggestions]).map((value) => ({
      label: value,
      value,
    })),
    { label: ANOTHER_MODEL_OPTION_LABEL, value: CUSTOM_TARGET_MODEL_VALUE },
  ];
}

/** Identify values that require the explicit custom target-model editor. */
export function isCustomTargetModel(value: string, suggestions: readonly string[] = []): boolean {
  if (!value) return false;
  const knownModels = new Set(
    uniqueModelNames([...TARGET_MODEL_OPTIONS, ...suggestions]).map((model) =>
      model.toLocaleLowerCase(),
    ),
  );
  return !knownModels.has(value.trim().toLocaleLowerCase());
}

/** Combine curated, discovered, and already-authored supported model names in stable order. */
export function supportedModelSuggestions(
  suggestions: readonly string[],
  selected: readonly string[],
): string[] {
  return uniqueModelNames([...TARGET_MODEL_OPTIONS, ...suggestions, ...selected]);
}

/** Preserve vocabulary order while removing blank and case-insensitive duplicate names. */
function uniqueModelNames(values: readonly string[]): string[] {
  const seen = new Set<string>();
  const unique: string[] = [];
  for (const value of values) {
    const normalized = value.trim();
    const key = normalized.toLocaleLowerCase();
    if (!normalized || seen.has(key)) continue;
    seen.add(key);
    unique.push(normalized);
  }
  return unique;
}
