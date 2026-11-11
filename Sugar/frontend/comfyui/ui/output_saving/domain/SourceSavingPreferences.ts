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
/** Define portable workflow-local output choices independently from rendering and graph mutation. */
import { isRecord } from '../../types/common.js';

export interface SourceSavingPreference {
  readonly enabled: boolean;
  readonly label: string | null;
}

/** Validate the versioned persisted contract before it influences saving or display labels. */
export function decodeSourceSavingPreferences(
  value: unknown,
): ReadonlyMap<string, SourceSavingPreference> {
  if (value === undefined || value === null) return new Map();
  if (
    !isRecord(value) ||
    Array.isArray(value) ||
    value.schema_version !== 1 ||
    !isRecord(value.sources) ||
    Array.isArray(value.sources) ||
    Object.keys(value.sources).length > 10000
  )
    throw new Error('Unsupported workflow output-source preferences.');
  const preferences = new Map<string, SourceSavingPreference>();
  for (const [path, entry] of Object.entries(value.sources)) {
    if (
      !path ||
      path.length > 4096 ||
      path.includes('\0') ||
      !isRecord(entry) ||
      Array.isArray(entry)
    )
      throw new Error('Invalid output source preference.');
    const enabled = entry.enabled === undefined ? true : entry.enabled;
    const label = entry.label ?? null;
    if (
      typeof enabled !== 'boolean' ||
      (label !== null && (typeof label !== 'string' || label.length > 4096 || label.includes('\0')))
    )
      throw new Error('Invalid output source preference.');
    preferences.set(
      path,
      Object.freeze({ enabled, label: typeof label === 'string' ? label.trim() || null : null }),
    );
  }
  return preferences;
}

/** Preserve stable execution identities without editing a shared subgraph definition. */
export function encodeSourceSavingPreferences(
  preferences: ReadonlyMap<string, SourceSavingPreference>,
): object {
  return { schema_version: 1, sources: Object.fromEntries(preferences) };
}

/** Read one workflow-owned namespace while retaining unrelated workflow metadata. */
export function readWorkflowSourceSavingPreferences(
  workflow: unknown,
): ReadonlyMap<string, SourceSavingPreference> {
  const extra = isRecord(workflow) && isRecord(workflow.extra) ? workflow.extra : null;
  const header = extra?.sugarcubes_output_saving;
  if (header === undefined || header === null) return new Map();
  if (!isRecord(header) || Array.isArray(header))
    throw new Error('Workflow output-saving metadata must be an object.');
  return decodeSourceSavingPreferences(header.source_preferences);
}
