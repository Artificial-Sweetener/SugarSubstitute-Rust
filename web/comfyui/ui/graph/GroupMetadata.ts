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
 * Own SugarCubes managed group metadata.
 */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGroup, CubeSurface } from '../types/graph.js';

export interface CubeMarkerLookup {
  inputs: string[];
  outputs: string[];
}

export interface CubeDefinitionMetadata {
  cube_id: string;
  default_alias: string;
  target_model: string;
  cube_version: string;
  cube_revision_ref: string;
  cube_definition_key: string;
  surface_signature: string;
  surface: CubeSurface | null;
  icon?: UnknownRecord;
}

export interface CubeInstanceMetadata {
  instance_id: string;
  instance_alias: string;
  nodes: string[];
  markers: CubeMarkerLookup;
  bounds: UnknownRecord;
}

export interface CubePresetMetadata {
  flavor: string | null;
  flavor_scope: string;
  active_flavor_values: UnknownRecord;
}

export interface CubeRuntimeMetadata {
  implementation_dirty: boolean;
  surface_values_changed: boolean;
  cosmetic_dirty: boolean;
  has_saveable_changes: boolean;
  dirty: boolean;
  dirty_at: string | null;
}

export interface CubeGroupMetadata {
  schema: number;
  managed: boolean;
  definition: CubeDefinitionMetadata;
  instance: CubeInstanceMetadata;
  preset: CubePresetMetadata;
  runtime: CubeRuntimeMetadata;
}

export interface CubeGroupMetadataRecord extends UnknownRecord {
  schema?: unknown;
  managed?: unknown;
  cube_id?: unknown;
  default_alias?: unknown;
  target_model?: unknown;
  cube_version?: unknown;
  cube_revision_ref?: unknown;
  cube_definition_key?: unknown;
  surface_signature?: unknown;
  surface?: unknown;
  icon?: unknown;
  instance_id?: unknown;
  instance_alias?: unknown;
  nodes?: unknown;
  markers?: unknown;
  bounds?: unknown;
  definition?: unknown;
  instance?: unknown;
  preset?: unknown;
  runtime?: unknown;
}

interface DisplayNameOptions {
  metadata?: CubeGroupMetadataRecord | null;
  group?: ComfyGroup | null;
  fallback?: unknown;
}

interface GroupPrototype extends ComfyGroup {
  __sugarcubes_serialized?: boolean;
  serialize?(): UnknownRecord;
  configure?(info: UnknownRecord): void;
}

interface GroupConstructor {
  prototype: GroupPrototype;
}

interface GroupSerializationAdapter {
  getLiteGraph?(): { LGraphGroup?: GroupConstructor } | null;
}

function cloneJson<T>(value: unknown, fallback: T): T {
  if (value == null) {
    return fallback;
  }
  try {
    const cloned: unknown = JSON.parse(JSON.stringify(value));
    return cloned as T;
  } catch (_error) {
    return fallback;
  }
}

function readObject(value: unknown): UnknownRecord {
  return isRecord(value) && !Array.isArray(value) ? value : {};
}

function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function readBoolean(value: unknown): boolean {
  return Boolean(value);
}

function readMarkerList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry) => entry != null).map((entry) => String(entry))
    : [];
}

function readMarkerLookup(markers: unknown): CubeMarkerLookup {
  if (Array.isArray(markers)) {
    return {
      inputs: readMarkerList(markers),
      outputs: [],
    };
  }
  const markerObject = readObject(markers);
  return {
    inputs: readMarkerList(markerObject.inputs),
    outputs: readMarkerList(markerObject.outputs),
  };
}

function readActiveFlavorValues(value: unknown): UnknownRecord {
  return cloneJson(readObject(value), {} as UnknownRecord);
}

function readBounds(value: unknown): UnknownRecord {
  const bounds = readObject(value);
  return Object.keys(bounds).length ? cloneJson(bounds, {} as UnknownRecord) : {};
}

function readIcon(value: unknown): UnknownRecord | null {
  const icon = readObject(value);
  return Object.keys(icon).length ? cloneJson(icon, {} as UnknownRecord) : null;
}

function readNodeIds(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry) => entry != null).map((entry) => String(entry))
    : [];
}

function flattenMetadataSections(metadata: unknown): CubeGroupMetadataRecord {
  const normalized = normalizeCubeGroupMetadata(metadata);
  const flattened: CubeGroupMetadataRecord = {
    schema: normalized.schema,
    managed: normalized.managed,
    cube_id: normalized.definition.cube_id,
    default_alias: normalized.definition.default_alias,
    target_model: normalized.definition.target_model,
    cube_version: normalized.definition.cube_version,
    cube_revision_ref: normalized.definition.cube_revision_ref,
    cube_definition_key: normalized.definition.cube_definition_key,
    surface_signature: normalized.definition.surface_signature,
    surface: cloneJson(normalized.definition.surface, null),
    instance_id: normalized.instance.instance_id,
    instance_alias: normalized.instance.instance_alias,
    nodes: cloneJson(normalized.instance.nodes, [] as string[]),
    markers: cloneJson(normalized.instance.markers, { inputs: [], outputs: [] }),
    bounds: cloneJson(normalized.instance.bounds, {} as UnknownRecord),
    flavor: normalized.preset.flavor,
    flavor_scope: normalized.preset.flavor_scope,
    active_flavor_values: cloneJson(normalized.preset.active_flavor_values, {} as UnknownRecord),
    implementation_dirty: normalized.runtime.implementation_dirty,
    surface_values_changed: normalized.runtime.surface_values_changed,
    cosmetic_dirty: normalized.runtime.cosmetic_dirty,
    has_saveable_changes: normalized.runtime.has_saveable_changes,
    dirty: normalized.runtime.dirty,
    dirty_at: normalized.runtime.dirty_at,
  };
  if (normalized.definition.icon) {
    flattened.icon = cloneJson(normalized.definition.icon, null);
  }
  return flattened;
}

/** Flatten owned metadata sections while preserving unrelated extension fields. */
export function flattenCubeGroupMetadata(
  metadata: unknown,
  preserved: unknown = {},
): CubeGroupMetadataRecord {
  return {
    ...readObject(preserved),
    ...flattenMetadataSections(metadata),
  };
}

/**
 * Normalize cube group metadata into explicit ownership sections.
 */
export function normalizeCubeGroupMetadata(raw: unknown): CubeGroupMetadata {
  const metadata = readObject(raw);
  const definition = readObject(metadata.definition);
  const instance = readObject(metadata.instance);
  const preset = readObject(metadata.preset);
  const runtime = readObject(metadata.runtime);
  const resolvedDefinition: CubeDefinitionMetadata = {
    cube_id: readString(definition.cube_id) || readString(metadata.cube_id),
    default_alias: readString(definition.default_alias) || readString(metadata.default_alias),
    target_model: readString(definition.target_model) || readString(metadata.target_model),
    cube_version: readString(definition.cube_version) || readString(metadata.cube_version),
    cube_revision_ref:
      readString(definition.cube_revision_ref) || readString(metadata.cube_revision_ref),
    cube_definition_key:
      readString(definition.cube_definition_key) || readString(metadata.cube_definition_key),
    surface_signature:
      readString(definition.surface_signature) || readString(metadata.surface_signature),
    surface:
      definition.surface && typeof definition.surface === 'object'
        ? (cloneJson(definition.surface, null) as CubeSurface | null)
        : (cloneJson(metadata.surface, null) as CubeSurface | null),
  };
  const resolvedIcon =
    readIcon(definition.icon) ||
    readIcon(metadata.icon) ||
    readIcon(readObject(metadata.metadata).icon);
  if (resolvedIcon) {
    resolvedDefinition.icon = resolvedIcon;
  }
  const resolvedInstance = {
    instance_id: readString(instance.instance_id) || readString(metadata.instance_id),
    instance_alias: readString(instance.instance_alias) || readString(metadata.instance_alias),
    nodes: readNodeIds(instance.nodes || metadata.nodes),
    markers: readMarkerLookup(instance.markers || metadata.markers),
    bounds: readBounds(instance.bounds || metadata.bounds),
  };
  const resolvedPreset = {
    flavor: readString(preset.flavor) || readString(metadata.flavor) || null,
    flavor_scope:
      readString(preset.flavor_scope) || readString(metadata.flavor_scope) || 'authored',
    active_flavor_values: readActiveFlavorValues(
      Object.prototype.hasOwnProperty.call(preset, 'active_flavor_values')
        ? preset.active_flavor_values
        : metadata.active_flavor_values,
    ),
  };
  const implementationDirty = Object.prototype.hasOwnProperty.call(runtime, 'implementation_dirty')
    ? readBoolean(runtime.implementation_dirty)
    : readBoolean(metadata.implementation_dirty ?? metadata.dirty);
  const surfaceValuesChanged = Object.prototype.hasOwnProperty.call(
    runtime,
    'surface_values_changed',
  )
    ? readBoolean(runtime.surface_values_changed)
    : readBoolean(metadata.surface_values_changed);
  const cosmeticDirty = Object.prototype.hasOwnProperty.call(runtime, 'cosmetic_dirty')
    ? readBoolean(runtime.cosmetic_dirty)
    : readBoolean(metadata.cosmetic_dirty);
  const hasSaveableChanges = Object.prototype.hasOwnProperty.call(runtime, 'has_saveable_changes')
    ? readBoolean(runtime.has_saveable_changes)
    : readBoolean(metadata.has_saveable_changes);
  const resolvedRuntime = {
    implementation_dirty: implementationDirty,
    surface_values_changed: surfaceValuesChanged,
    cosmetic_dirty: cosmeticDirty,
    has_saveable_changes: hasSaveableChanges,
    dirty: Object.prototype.hasOwnProperty.call(runtime, 'dirty')
      ? readBoolean(runtime.dirty)
      : readBoolean(metadata.dirty ?? implementationDirty),
    dirty_at: readString(runtime.dirty_at) || readString(metadata.dirty_at) || null,
  };

  return {
    schema: Number.isFinite(Number(metadata.schema)) ? Number(metadata.schema) : 6,
    managed: Object.prototype.hasOwnProperty.call(metadata, 'managed')
      ? readBoolean(metadata.managed)
      : true,
    definition: resolvedDefinition,
    instance: resolvedInstance,
    preset: resolvedPreset,
    runtime: resolvedRuntime,
  };
}

/**
 * Read cube definition metadata.
 */
export function readCubeDefinitionMetadata(metadata: unknown): CubeDefinitionMetadata {
  return normalizeCubeGroupMetadata(metadata).definition;
}

/**
 * Read cube instance metadata.
 */
export function readCubeInstanceMetadata(metadata: unknown): CubeInstanceMetadata {
  return normalizeCubeGroupMetadata(metadata).instance;
}

/**
 * Read cube preset metadata.
 */
export function readCubePresetMetadata(metadata: unknown): CubePresetMetadata {
  return normalizeCubeGroupMetadata(metadata).preset;
}

/**
 * Read cube runtime metadata.
 */
export function readCubeRuntimeMetadata(metadata: unknown): CubeRuntimeMetadata {
  return normalizeCubeGroupMetadata(metadata).runtime;
}

/**
 * Write cube definition metadata.
 */
export function writeCubeDefinitionMetadata(
  metadata: unknown,
  patch: Partial<CubeDefinitionMetadata> = {},
): CubeGroupMetadata {
  const normalized = normalizeCubeGroupMetadata(metadata);
  return {
    ...normalized,
    definition: { ...normalized.definition, ...readObject(patch) },
  };
}

/**
 * Write cube instance metadata.
 */
export function writeCubeInstanceMetadata(
  metadata: unknown,
  patch: Partial<CubeInstanceMetadata> = {},
): CubeGroupMetadata {
  const normalized = normalizeCubeGroupMetadata(metadata);
  return {
    ...normalized,
    instance: { ...normalized.instance, ...readObject(patch) },
  };
}

/**
 * Write cube preset metadata.
 */
export function writeCubePresetMetadata(
  metadata: unknown,
  patch: Partial<CubePresetMetadata> = {},
): CubeGroupMetadata {
  const normalized = normalizeCubeGroupMetadata(metadata);
  return {
    ...normalized,
    preset: { ...normalized.preset, ...readObject(patch) },
  };
}

/**
 * Write cube runtime metadata.
 */
export function writeCubeRuntimeMetadata(
  metadata: unknown,
  patch: Partial<CubeRuntimeMetadata> = {},
): CubeGroupMetadata {
  const normalized = normalizeCubeGroupMetadata(metadata);
  return {
    ...normalized,
    runtime: { ...normalized.runtime, ...readObject(patch) },
  };
}

/**
 * Serialize group metadata for workflow persistence.
 */
export function serializeCubeGroupMetadataForWorkflow(metadata: unknown): unknown {
  const cloned = cloneJson(metadata, null as unknown);
  return cloned || flattenMetadataSections(metadata);
}

/**
 * Serialize group metadata for cube layout persistence.
 */
export function serializeCubeGroupMetadataForCubeLayout(metadata: unknown): UnknownRecord {
  const normalized = normalizeCubeGroupMetadata(metadata);
  const layoutMetadata: UnknownRecord = {
    schema: normalized.schema,
    managed: normalized.managed,
    cube_id: normalized.definition.cube_id,
    default_alias: normalized.definition.default_alias,
    target_model: normalized.definition.target_model,
    cube_version: normalized.definition.cube_version,
    cube_revision_ref: normalized.definition.cube_revision_ref,
    cube_definition_key: normalized.definition.cube_definition_key,
    surface_signature: normalized.definition.surface_signature,
    markers: cloneJson(normalized.instance.markers, { inputs: [], outputs: [] }),
    nodes: cloneJson(normalized.instance.nodes, [] as string[]),
  };
  if (normalized.definition.icon) {
    layoutMetadata.icon = cloneJson(normalized.definition.icon, null);
  }
  if (Object.keys(normalized.instance.bounds).length) {
    layoutMetadata.bounds = cloneJson(normalized.instance.bounds, {} as UnknownRecord);
  }
  return Object.fromEntries(
    Object.entries(layoutMetadata).filter(
      ([_key, value]) =>
        value !== '' &&
        value != null &&
        (!Array.isArray(value) || value.length > 0) &&
        (typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length > 0),
    ),
  );
}

/**
 * Get group sugarcubes.
 */
export function getGroupSugarcubes(
  group: ComfyGroup | null | undefined,
): CubeGroupMetadataRecord | null {
  if (!group || typeof group !== 'object') {
    return null;
  }
  if (group.properties && typeof group.properties === 'object' && group.properties.sugarcubes) {
    return isRecord(group.properties.sugarcubes)
      ? (group.properties.sugarcubes as CubeGroupMetadataRecord)
      : null;
  }
  return null;
}

/**
 * Set group sugarcubes.
 */
export function setGroupSugarcubes(
  group: ComfyGroup | null | undefined,
  data: CubeGroupMetadataRecord | CubeGroupMetadata,
): void {
  if (!group || typeof group !== 'object') {
    return;
  }
  if (!group.properties || typeof group.properties !== 'object') {
    group.properties = {};
  }
  group.properties.sugarcubes = data;
}

/**
 * Normalize graph-local instance alias.
 */
export function normalizeGroupInstanceAlias(
  group: ComfyGroup | null | undefined,
  metadata: CubeGroupMetadataRecord | null | undefined,
  fallbackInstanceAlias: unknown = '',
): { instanceAlias: string; metadata: CubeGroupMetadataRecord | null } {
  const safeGroup = group && typeof group === 'object' ? group : null;
  const next: CubeGroupMetadataRecord | null = metadata ? { ...metadata } : null;
  let instanceAlias = '';

  if (typeof next?.instance_alias === 'string' && next.instance_alias.trim()) {
    instanceAlias = next.instance_alias.trim();
  } else if (typeof fallbackInstanceAlias === 'string' && fallbackInstanceAlias.trim()) {
    instanceAlias = fallbackInstanceAlias.trim();
  } else if (typeof next?.default_alias === 'string' && next.default_alias.trim()) {
    instanceAlias = next.default_alias.trim();
  }

  if (next && instanceAlias) {
    next.instance_alias = instanceAlias;
  }
  if (safeGroup && instanceAlias && safeGroup.title !== instanceAlias) {
    safeGroup.title = instanceAlias;
  }

  return { instanceAlias, metadata: next };
}

/**
 * Resolve graph-local instance display name.
 */
export function resolveInstanceDisplayName({
  metadata,
  group,
  fallback,
}: DisplayNameOptions = {}): string {
  const instanceAlias =
    typeof metadata?.instance_alias === 'string' ? metadata.instance_alias.trim() : '';
  if (instanceAlias) {
    return instanceAlias;
  }
  const defaultAlias =
    typeof metadata?.default_alias === 'string' ? metadata.default_alias.trim() : '';
  if (defaultAlias) {
    return defaultAlias;
  }
  const title = typeof group?.title === 'string' ? group.title.trim() : '';
  if (title) {
    return title;
  }
  const fallbackValue = typeof fallback === 'string' ? fallback.trim() : '';
  return fallbackValue || '';
}

/**
 * Resolve cube-level display name.
 */
export function resolveCubeDisplayName({
  metadata,
  group,
  fallback,
}: DisplayNameOptions = {}): string {
  const defaultAlias =
    typeof metadata?.default_alias === 'string' ? metadata.default_alias.trim() : '';
  if (defaultAlias) {
    return defaultAlias;
  }
  const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id.trim() : '';
  if (cubeId) {
    return cubeId;
  }
  const title = typeof group?.title === 'string' ? group.title.trim() : '';
  if (title) {
    return title;
  }
  const fallbackValue = typeof fallback === 'string' ? fallback.trim() : '';
  return fallbackValue || '';
}

/**
 * Ensure group serialization.
 */
export function ensureGroupSerialization(
  adapter: GroupSerializationAdapter | null | undefined,
): void {
  const liteGraph = adapter?.getLiteGraph?.() || null;
  const GroupRef = liteGraph?.LGraphGroup;
  if (!GroupRef || GroupRef.prototype.__sugarcubes_serialized) {
    return;
  }
  GroupRef.prototype.__sugarcubes_serialized = true;
  const originalSerialize = GroupRef.prototype.serialize;
  GroupRef.prototype.serialize = function serializeSugarCubesGroup(this: GroupPrototype) {
    const payload =
      typeof originalSerialize === 'function' ? originalSerialize.call(this) : { id: this.id };
    const sugarcubes = getGroupSugarcubes(this);
    if (sugarcubes) {
      payload.sugarcubes = sugarcubes;
    }
    return payload;
  };
  const originalConfigure = GroupRef.prototype.configure;
  GroupRef.prototype.configure = function configureSugarCubesGroup(
    this: GroupPrototype,
    info: UnknownRecord,
  ) {
    if (typeof originalConfigure === 'function') {
      originalConfigure.call(this, info);
    }
    if (isRecord(info.sugarcubes)) {
      setGroupSugarcubes(this, info.sugarcubes as CubeGroupMetadataRecord);
    }
  };
}
