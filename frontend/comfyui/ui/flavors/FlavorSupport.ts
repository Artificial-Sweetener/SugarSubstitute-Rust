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
/** Define shared Flavor contracts and normalization policy. */

import { filterTrackedSurfaceValues } from '../core/SurfaceValuePolicy.js';
import { readWidgetValue, writeWidgetValue } from '../graph/Markers.js';
import { normalizeAuthoredFlavors, normalizeFlavorId } from './FlavorSelection.js';
import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { FlavorOption } from './FlavorSelection.js';
import type {
  ComfyApplication,
  ComfyGraph,
  ComfyNode,
  CubeSurface,
  GraphId,
  SurfaceControl,
} from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';

export interface NormalizedSurface extends CubeSurface {
  default_flavor_id: string;
  controls: SurfaceControl[];
}

export interface FlavorMetadata extends CubeGroupMetadataRecord {
  managed?: boolean;
  cube_id?: string;
  cube_version?: string;
  cube_definition_key?: string;
  surface_signature?: string;
  surface?: NormalizedSurface;
  instance_id?: string;
  instance_alias?: string;
  default_alias?: string;
  nodes?: GraphId[];
  authored_flavors?: FlavorOption[];
  local_flavors?: FlavorOption[];
  flavor_options?: FlavorOption[];
  flavors?: FlavorOption[] | string[];
  flavor?: string;
  flavor_scope?: string;
  active_flavor_values?: UnknownRecord;
}

export interface CubeFlavorPayload extends UnknownRecord {
  version?: unknown;
  surface?: unknown;
  surface_signature?: unknown;
  flavors?: { authored?: unknown };
}

export interface FlavorServiceAdapter {
  getApp?(): ComfyApplication | null;
}
export interface FlavorDialogs {
  promptText?(options: UnknownRecord): Promise<string | null>;
  selectItem?(options: UnknownRecord): Promise<string | null>;
}
export interface FlavorEvents {
  on?(event: string, listener: (payload: UnknownRecord) => void): () => void;
}
export interface FlavorToast {
  push?(severity: string, summary: string, detail: string): void;
}
export interface FlavorApi {
  saveAuthoredFlavor?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
  getLocalFlavors?(cubeId: string): Promise<ApiJsonResult>;
  saveLocalFlavor?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
  deleteLocalFlavor?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
  selectLocalFlavor?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
  reconcileLocalFlavors?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
  migrateLocalFlavors?(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
}
export interface FlavorDirtyManager {
  requestRefresh?(options: { graph: ComfyGraph; reason: string }): void;
}
export interface FlavorBrowser {
  refresh?(options: { force: boolean }): Promise<unknown>;
}
export interface FlavorStorageAdapter {
  readValue?(key: string): string | null;
  writeValue?(key: string, value: unknown): void;
  readJson?(key: string): UnknownRecord | null;
}
export interface FlavorServiceOptions {
  adapter?: FlavorServiceAdapter | null;
  dialogs?: FlavorDialogs | null;
  events?: FlavorEvents | null;
  storage?: FlavorStorageAdapter | null;
  toast?: FlavorToast | null;
  api?: FlavorApi | null;
  dirtyManager?: FlavorDirtyManager | null;
  cubeBrowser?: FlavorBrowser | null;
}
export interface HydrateOptions {
  cubeId?: string;
  definitionKey?: string;
  entry?: unknown;
  graph?: ComfyGraph;
  forceApply?: boolean;
}
export interface ImportedFlavorMetadata {
  surface: NormalizedSurface;
  surface_signature: string;
  authored_flavors: FlavorOption[];
  flavor: 'default';
  flavor_scope: 'authored';
  active_flavor_values: UnknownRecord;
  flavor_options: FlavorOption[];
  flavors: string[];
  local_flavors: FlavorOption[];
}

/** Normalize unknown group metadata to the Flavor metadata contract. */
export function asFlavorMetadata(value: unknown): FlavorMetadata {
  return isRecord(value) ? (value as FlavorMetadata) : {};
}
/** Read a useful error message with a stable fallback. */
export function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}
/** Read a structured Flavor API response error. */
export function responseErrorMessage(data: UnknownRecord): string {
  const error = isRecord(data.error) ? data.error : {};
  return typeof error.message === 'string' ? error.message : '';
}
/** Read the saved Flavor identity from an API response. */
export function savedFlavorId(data: UnknownRecord): string {
  const saved = isRecord(data.saved) ? data.saved : {};
  return typeof saved.flavor_id === 'string' ? saved.flavor_id : '';
}
/** Clone JSON-compatible Flavor values before crossing ownership boundaries. */
export function cloneValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
/** Read the property fallback for one surface input. */
export function readNodePropertyValue(node: ComfyNode, inputName: string): unknown {
  return node.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName)
    ? node.properties[inputName]
    : null;
}
/** Apply one Flavor value through the host's preferred node boundary. */
export function applyNodeValue(node: ComfyNode, inputName: string, value: unknown): boolean {
  if (writeWidgetValue(node, inputName, value)) return true;
  if (typeof node.setProperty === 'function') {
    try {
      node.setProperty(inputName, value);
      return true;
    } catch (_error) {
      // Fall through to direct property ownership.
    }
  }
  if (!node.properties || typeof node.properties !== 'object') node.properties = {};
  node.properties[inputName] = value;
  if (typeof node.onPropertyChanged === 'function') {
    try {
      node.onPropertyChanged(inputName, value);
    } catch (_error) {
      // Host callbacks cannot invalidate the applied property value.
    }
  }
  return true;
}
/** Normalize a Cube surface for Flavor projection. */
export function normalizeSurface(surface: unknown): NormalizedSurface {
  if (!isRecord(surface)) return { default_flavor_id: 'default', controls: [] };
  return {
    default_flavor_id:
      typeof surface.default_flavor_id === 'string' && surface.default_flavor_id.trim()
        ? surface.default_flavor_id.trim()
        : 'default',
    controls: Array.isArray(surface.controls) ? surface.controls : [],
  };
}
/** Normalize authored Flavors and retain only tracked surface values. */
export function normalizeAuthoredFlavorEntries(
  flavors: unknown,
  surface: NormalizedSurface = normalizeSurface(null),
): FlavorOption[] {
  return normalizeAuthoredFlavors(flavors, surface.default_flavor_id).map((entry) => ({
    ...entry,
    values: filterTrackedSurfaceValues(surface, entry.values),
  }));
}
/** Normalize local Flavors and retain only tracked surface values. */
export function normalizeLocalFlavorEntries(flavors: unknown, surface: unknown): FlavorOption[] {
  return (Array.isArray(flavors) ? flavors : []).flatMap((entry): FlavorOption[] => {
    if (!isRecord(entry) || typeof entry.id !== 'string' || !entry.id) return [];
    return [
      {
        id: entry.id,
        name: typeof entry.name === 'string' ? entry.name : entry.id,
        scope: 'local',
        stale: Boolean(entry.stale),
        values: filterTrackedSurfaceValues(normalizeSurface(surface), entry.values),
      },
    ];
  });
}
/** Resolve the established dialog name seed from instance metadata. */
export function resolveFlavorNameSeed(metadata: FlavorMetadata): string {
  return (
    metadata.instance_alias?.trim() ||
    metadata.default_alias?.trim() ||
    metadata.cube_id?.trim() ||
    ''
  );
}
/** Normalize a Flavor name for collision comparison. */
export function normalizeFlavorNameKey(value: unknown): string {
  return String(value || '')
    .trim()
    .toLowerCase();
}
/** Build normalized ID and name sets for Flavor collision policy. */
export function flavorKeySets(flavors: unknown): { ids: Set<string>; names: Set<string> } {
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const flavor of Array.isArray(flavors) ? flavors : []) {
    const id = normalizeFlavorId(flavor?.id);
    const name = normalizeFlavorNameKey(flavor?.name);
    if (id) ids.add(id);
    if (name) names.add(name);
  }
  return { ids, names };
}
/** Return local Flavors that collide with authored identities or names. */
export function findLocalFlavorCollisions(
  local: FlavorOption[],
  authored: FlavorOption[],
): FlavorOption[] {
  const keys = flavorKeySets(authored);
  return local.filter((flavor) => {
    const id = normalizeFlavorId(flavor.id);
    const name = normalizeFlavorNameKey(flavor.name);
    return Boolean((id && keys.ids.has(id)) || (name && keys.names.has(name)));
  });
}

export { readWidgetValue };
