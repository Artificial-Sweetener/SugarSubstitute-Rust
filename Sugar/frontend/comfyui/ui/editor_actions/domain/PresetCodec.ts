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
/** Validate the external Substitute preset schema without weakening internal contracts. */
import { isRecord } from '../../types/common.js';
import type { PresetAssociation, PresetScope, PresetValue, UserPreset } from './UserPreset.js';
const scopes: readonly PresetScope[] = [
  'global',
  'model_family',
  'provider_model',
  'provider_model_version',
  'local_model',
];

/** Distinguish routine rejected values from unexpected getter or runtime failures. */
class PresetValueValidationError extends Error {}

/** Return a detached savable input while omitting values rejected by the shared wire policy. */
export function copySavablePresetValue(value: unknown): PresetValue | undefined {
  try {
    return copyPresetValue(value);
  } catch (error: unknown) {
    if (error instanceof PresetValueValidationError) return undefined;
    throw error;
  }
}

/** Reject unsupported or damaged libraries so later saves cannot erase unknown data. */
export function decodePresetDocument(value: unknown): UserPreset[] {
  if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.presets))
    throw new Error('Unsupported preset library.');
  const presets = value.presets.map(decodePreset);
  if (new Set(presets.map((preset) => preset.id)).size !== presets.length)
    throw new Error('Duplicate preset identity.');
  return presets;
}

/** Produce the same versioned wire fields used by Substitute's presets.json. */
export function encodePresetDocument(presets: readonly UserPreset[]): {
  version: 1;
  presets: readonly UserPreset[];
} {
  return { version: 1, presets: decodePresetDocument({ version: 1, presets }) };
}

/** Detach bounded JSON values while rejecting graph connections and invalid object keys. */
export function copyPresetValue(value: unknown, depth = 0): PresetValue {
  if (depth > 64) throw new PresetValueValidationError('Preset value is too deeply nested.');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (Array.isArray(value)) {
    if (value.length === 2 && typeof value[0] === 'string' && Number.isInteger(value[1]))
      throw new PresetValueValidationError('Presets cannot contain graph connections.');
    return value.map((item: unknown) => copyPresetValue(item, depth + 1));
  }
  if (!isRecord(value)) throw new PresetValueValidationError('Preset value must be JSON.');
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => {
      if (!key.trim() || ['__proto__', 'constructor', 'prototype'].includes(key))
        throw new PresetValueValidationError('Invalid preset input key.');
      return [key, copyPresetValue(item, depth + 1)];
    }),
  );
}

/** Validate one association while retaining every supported imported scope. */
function decodeAssociation(value: unknown): PresetAssociation {
  if (!isRecord(value)) throw new Error('Invalid preset association.');
  const scope = scopes.find((scope) => scope === value.scope);
  if (!scope) throw new Error('Invalid preset scope.');
  const key = text(value.key),
    label = text(value.label);
  const provider = value.provider === null ? null : text(value.provider);
  if (scope === 'global' ? provider !== null || key !== 'global' : provider === null)
    throw new Error('Invalid preset association identity.');
  return { scope, provider, key, label };
}

/** Decode records including prompt payloads, which are preserved but never exposed as editor actions. */
function decodePreset(value: unknown): UserPreset {
  if (!isRecord(value) || !isRecord(value.payload) || !Array.isArray(value.associations))
    throw new Error('Invalid preset record.');
  const record = {
    id: text(value.id),
    label: text(value.label),
    created_at: text(value.created_at),
    updated_at: text(value.updated_at),
    associations: value.associations.map(decodeAssociation),
  };
  const payload = value.payload;
  if (
    value.kind === 'dimension' &&
    positiveInteger(payload.short_edge) &&
    positiveInteger(payload.long_edge) &&
    payload.short_edge <= payload.long_edge
  )
    return {
      ...record,
      kind: 'dimension',
      payload: { short_edge: payload.short_edge, long_edge: payload.long_edge },
    };
  if (
    value.kind === 'node_inputs' &&
    isRecord(payload.inputs) &&
    Object.keys(payload.inputs).length
  ) {
    const inputs = Object.fromEntries(
      Object.entries(payload.inputs).map(([key, item]) => {
        if (!key.trim() || ['__proto__', 'constructor', 'prototype'].includes(key))
          throw new Error('Invalid preset input key.');
        return [key, copyPresetValue(item)];
      }),
    );
    return {
      ...record,
      kind: 'node_inputs',
      payload: { node_type: text(payload.node_type), inputs },
    };
  }
  if (value.kind === 'prompt_string')
    return { ...record, kind: 'prompt_string', payload: { text: text(payload.text) } };
  throw new Error('Invalid preset payload.');
}
/** Require non-empty identity text without coercing external data. */
function text(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) throw new Error('Invalid preset text.');
  return value;
}
/** Narrow positive persisted dimension edges without accepting booleans or floats. */
function positiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}
