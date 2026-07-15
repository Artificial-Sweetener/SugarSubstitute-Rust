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
 * Own the SugarCubes flavor domain orchestration layer in
 * `frontend/comfyui/ui/flavors/FlavorService.js`.
 */

import { getGraphGroups } from '../graph/GraphQuery.js';
import {
  flattenCubeGroupMetadata,
  getGroupSugarcubes,
  setGroupSugarcubes,
  writeCubeDefinitionMetadata,
  writeCubePresetMetadata,
} from '../graph/GroupMetadata.js';
import { readWidgetValue, writeWidgetValue } from '../graph/Markers.js';
import { buildSurfaceNodesBySymbol } from '../graph/SurfaceNodeResolver.js';
import { buildCubeDefinitionKey } from '../core/CubeDefinitionKey.js';
import { filterTrackedSurfaceValues, trackedSurfaceControls } from '../core/SurfaceValuePolicy.js';
import { FlavorStorage } from './FlavorStorage.js';
import {
  defaultAuthoredFlavor,
  defaultsOnlyFlavorOptions,
  normalizeFlavorId,
  normalizeAuthoredFlavors,
} from './FlavorSelection.js';
import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { FlavorOption } from './FlavorSelection.js';
import type {
  ComfyApplication,
  ComfyGraph,
  ComfyGroup,
  ComfyNode,
  CubeSurface,
  GraphId,
  SurfaceControl,
} from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';

interface NormalizedSurface extends CubeSurface {
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

interface FlavorServiceAdapter {
  getApp?(): ComfyApplication | null;
}

interface FlavorDialogs {
  promptText?(options: UnknownRecord): Promise<string | null>;
  selectItem?(options: UnknownRecord): Promise<string | null>;
}

interface FlavorEvents {
  on?(event: string, listener: (payload: UnknownRecord) => void): () => void;
}

interface FlavorToast {
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

interface FlavorDirtyManager {
  requestRefresh?(options: { graph: ComfyGraph; reason: string }): void;
}

interface FlavorBrowser {
  refresh?(options: { force: boolean }): Promise<unknown>;
}

interface FlavorStorageAdapter {
  readValue?(key: string): string | null;
  writeValue?(key: string, value: unknown): void;
  readJson?(key: string): UnknownRecord | null;
}

interface FlavorServiceOptions {
  adapter?: FlavorServiceAdapter | null;
  dialogs?: FlavorDialogs | null;
  events?: FlavorEvents | null;
  storage?: FlavorStorageAdapter | null;
  toast?: FlavorToast | null;
  api?: FlavorApi | null;
  dirtyManager?: FlavorDirtyManager | null;
  cubeBrowser?: FlavorBrowser | null;
}

interface HydrateOptions {
  cubeId?: string;
  definitionKey?: string;
  entry?: unknown;
  graph?: ComfyGraph;
  forceApply?: boolean;
}

interface ImportedFlavorMetadata {
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

function asFlavorMetadata(value: unknown): FlavorMetadata {
  return isRecord(value) ? (value as FlavorMetadata) : {};
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

function responseErrorMessage(data: UnknownRecord): string {
  const error = isRecord(data.error) ? data.error : {};
  return typeof error.message === 'string' ? error.message : '';
}

function savedFlavorId(data: UnknownRecord): string {
  const saved = isRecord(data.saved) ? data.saved : {};
  return typeof saved.flavor_id === 'string' ? saved.flavor_id : '';
}

function cloneValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function readNodePropertyValue(node: ComfyNode, inputName: string): unknown {
  if (node?.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName)) {
    return node.properties[inputName];
  }
  return null;
}

function applyNodeValue(node: ComfyNode, inputName: string, value: unknown): boolean {
  if (writeWidgetValue(node, inputName, value)) {
    return true;
  }
  if (typeof node?.setProperty === 'function') {
    try {
      node.setProperty(inputName, value);
      return true;
    } catch (_error) {
      // ignore property setter failures
    }
  }
  if (!node?.properties || typeof node.properties !== 'object') {
    node.properties = {};
  }
  node.properties[inputName] = value;
  if (typeof node?.onPropertyChanged === 'function') {
    try {
      node.onPropertyChanged(inputName, value);
    } catch (_error) {
      // ignore property callback failures
    }
  }
  return true;
}

function normalizeSurface(surface: unknown): NormalizedSurface {
  if (!isRecord(surface)) {
    return { default_flavor_id: 'default', controls: [] };
  }
  return {
    default_flavor_id:
      typeof surface.default_flavor_id === 'string' && surface.default_flavor_id.trim()
        ? surface.default_flavor_id.trim()
        : 'default',
    controls: Array.isArray(surface.controls) ? surface.controls : [],
  };
}

function normalizeAuthoredFlavorEntries(
  flavors: unknown,
  surface: NormalizedSurface = normalizeSurface(null),
): FlavorOption[] {
  const normalized = normalizeAuthoredFlavors(flavors, surface?.default_flavor_id || 'default');
  return normalized.map((entry) => ({
    ...entry,
    values: filterTrackedSurfaceValues(surface, entry.values),
  }));
}

function normalizeLocalFlavorEntries(flavors: unknown, surface: unknown): FlavorOption[] {
  return (Array.isArray(flavors) ? flavors : []).flatMap((entry): FlavorOption[] => {
    if (!isRecord(entry)) {
      return [];
    }
    const id = typeof entry.id === 'string' ? entry.id : '';
    if (!id) {
      return [];
    }
    return [
      {
        id,
        name: typeof entry.name === 'string' ? entry.name : id,
        scope: 'local',
        stale: Boolean(entry.stale),
        values: filterTrackedSurfaceValues(normalizeSurface(surface), entry.values),
      },
    ];
  });
}

function resolveFlavorNameSeed(metadata: FlavorMetadata): string {
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
  const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id.trim() : '';
  return cubeId;
}

function normalizeFlavorNameKey(value: unknown): string {
  return String(value || '')
    .trim()
    .toLowerCase();
}

function flavorKeySets(flavors: unknown): { ids: Set<string>; names: Set<string> } {
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const flavor of Array.isArray(flavors) ? flavors : []) {
    const id = normalizeFlavorId(flavor?.id);
    const name = normalizeFlavorNameKey(flavor?.name);
    if (id) {
      ids.add(id);
    }
    if (name) {
      names.add(name);
    }
  }
  return { ids, names };
}

function findLocalFlavorCollisions(
  localFlavors: FlavorOption[],
  authoredFlavors: FlavorOption[],
): FlavorOption[] {
  const authoredKeys = flavorKeySets(authoredFlavors);
  return (Array.isArray(localFlavors) ? localFlavors : []).filter((flavor) => {
    const id = normalizeFlavorId(flavor?.id);
    const name = normalizeFlavorNameKey(flavor?.name);
    return (id && authoredKeys.ids.has(id)) || (name && authoredKeys.names.has(name));
  });
}

/**
 * Coordinate authored and local flavor behavior for managed instances.
 */
export class FlavorService {
  private readonly adapter: FlavorServiceAdapter | null;
  private readonly dialogs: FlavorDialogs | null;
  private readonly events: FlavorEvents | null;
  private readonly toast: FlavorToast | null;
  private readonly api: FlavorApi | null;
  private readonly dirtyManager: FlavorDirtyManager | null;
  private readonly cubeBrowser: FlavorBrowser | null;
  private readonly storage: FlavorStorage;
  private readonly unsubscribers: Array<() => void>;

  constructor({
    adapter,
    dialogs,
    events,
    storage,
    toast,
    api,
    dirtyManager,
    cubeBrowser,
  }: FlavorServiceOptions = {}) {
    this.adapter = adapter || null;
    this.dialogs = dialogs || null;
    this.events = events || null;
    this.toast = toast || null;
    this.api = api || null;
    this.dirtyManager = dirtyManager || null;
    this.cubeBrowser = cubeBrowser || null;
    this.storage = new FlavorStorage({
      ...(storage !== undefined ? { storage } : {}),
      ...(api !== undefined ? { api } : {}),
    });
    this.unsubscribers = [];
  }

  async setup(): Promise<void> {
    if (!this.events?.on) {
      return;
    }
    this.unsubscribers.push(
      this.events.on('cube:instances:updated', (payload) => {
        this.refreshGraph(isRecord(payload.graph) ? (payload.graph as ComfyGraph) : null);
      }),
    );
    this.unsubscribers.push(
      this.events.on('cube:flavor:change', (payload) => {
        this.selectFlavor({
          metadata: asFlavorMetadata(payload.metadata),
          flavor: payload.flavor,
        }).catch((error: unknown) => {
          this.toast?.push?.(
            'error',
            'Flavor selection failed',
            errorMessage(error, 'Local flavor selection could not be saved.'),
          );
        });
      }),
    );
    this.unsubscribers.push(
      this.events.on('cube:definition:loaded', (payload) => {
        const entry = isRecord(payload.entry)
          ? (payload.entry as HydrateOptions['entry'])
          : undefined;
        const graph = isRecord(payload.graph) ? (payload.graph as ComfyGraph) : undefined;
        this.hydrateFromDefinition({
          ...(typeof payload.cubeId === 'string' ? { cubeId: payload.cubeId } : {}),
          ...(typeof payload.definitionKey === 'string'
            ? { definitionKey: payload.definitionKey }
            : {}),
          ...(entry ? { entry } : {}),
          ...(graph ? { graph } : {}),
        }).catch((error: unknown) => {
          this.toast?.push?.(
            'warn',
            'Cube defaults unavailable',
            errorMessage(error, 'Cube default state could not be loaded.'),
          );
        });
      }),
    );
  }

  dispose(): void {
    for (const unsubscribe of this.unsubscribers.splice(0)) {
      try {
        unsubscribe?.();
      } catch (_error) {
        // ignore listener cleanup failures
      }
    }
  }

  buildImportedMetadata(cube: CubeFlavorPayload): ImportedFlavorMetadata {
    const surface = normalizeSurface(cube?.surface);
    const authoredFlavors = normalizeAuthoredFlavorEntries(cube?.flavors?.authored, surface);
    const selectedFlavor = defaultAuthoredFlavor(authoredFlavors);
    const activeValues = filterTrackedSurfaceValues(surface, selectedFlavor.values);
    const defaultOption = {
      ...selectedFlavor,
      values: cloneValue(activeValues),
    };
    return {
      surface,
      surface_signature: typeof cube?.surface_signature === 'string' ? cube.surface_signature : '',
      authored_flavors: authoredFlavors,
      flavor: 'default',
      flavor_scope: 'authored',
      active_flavor_values: cloneValue(activeValues),
      flavor_options: [defaultOption],
      flavors: [defaultOption.name],
      local_flavors: [],
    };
  }

  async hydrateFromDefinition({
    cubeId,
    definitionKey,
    entry,
    graph,
    forceApply = false,
  }: HydrateOptions = {}): Promise<number> {
    const entryRecord = isRecord(entry) ? entry : {};
    const payload = isRecord(entryRecord.payload) ? entryRecord.payload : {};
    const definitionCube = isRecord(payload.cube) ? (payload.cube as CubeFlavorPayload) : null;
    if (!graph || !cubeId || !definitionCube) {
      return 0;
    }
    const targetDefinitionKey =
      typeof definitionKey === 'string' && definitionKey.trim()
        ? definitionKey.trim()
        : buildCubeDefinitionKey(cubeId, definitionCube.version);
    const importedMetadata = this.buildImportedMetadata(definitionCube);
    let hydratedCount = 0;
    for (const group of getGraphGroups(graph)) {
      const metadata = asFlavorMetadata(getGroupSugarcubes(group));
      if (!metadata?.managed || metadata?.cube_id !== cubeId) {
        continue;
      }
      const hasVersionIdentity = Boolean(metadata?.cube_definition_key || metadata?.cube_version);
      const groupDefinitionKey =
        typeof metadata?.cube_definition_key === 'string' && metadata.cube_definition_key.trim()
          ? metadata.cube_definition_key.trim()
          : buildCubeDefinitionKey(metadata.cube_id, metadata.cube_version);
      if (
        hasVersionIdentity &&
        groupDefinitionKey &&
        targetDefinitionKey &&
        groupDefinitionKey !== targetDefinitionKey
      ) {
        continue;
      }
      const definitionMetadata = writeCubeDefinitionMetadata(metadata, {
        surface: importedMetadata.surface,
        surface_signature: importedMetadata.surface_signature,
      });
      const presetMetadata = writeCubePresetMetadata(definitionMetadata, {
        flavor: importedMetadata.flavor,
        flavor_scope: importedMetadata.flavor_scope,
        active_flavor_values: importedMetadata.active_flavor_values,
      });
      const nextMetadata = flattenCubeGroupMetadata(presetMetadata, {
        ...metadata,
        authored_flavors: importedMetadata.authored_flavors,
        flavor_options: importedMetadata.flavor_options,
        flavors: importedMetadata.flavors,
        local_flavors: importedMetadata.local_flavors,
      });
      setGroupSugarcubes(group, nextMetadata);
      this.refreshGroupMetadata(graph, group, asFlavorMetadata(getGroupSugarcubes(group)), {
        forceApply,
      });
      hydratedCount += 1;
    }
    return hydratedCount;
  }

  refreshGraph(graph: ComfyGraph | null): void {
    if (!graph) {
      return;
    }
    for (const group of getGraphGroups(graph)) {
      const metadata = asFlavorMetadata(getGroupSugarcubes(group));
      if (!metadata?.managed || !metadata?.instance_id || !metadata?.cube_id) {
        continue;
      }
      if (!Array.isArray(metadata?.authored_flavors) && !Array.isArray(metadata?.flavors)) {
        continue;
      }
      this.refreshGroupMetadata(graph, group, metadata);
    }
  }

  refreshGroupMetadata(
    graph: ComfyGraph,
    group: ComfyGroup | null,
    metadata: FlavorMetadata,
    { forceApply = false }: { forceApply?: boolean } = {},
  ): void {
    const authoredFlavors = normalizeAuthoredFlavorEntries(
      metadata?.authored_flavors || metadata?.flavors,
      metadata?.surface,
    );
    const options = defaultsOnlyFlavorOptions(authoredFlavors);
    const selectedFlavor: FlavorOption = {
      ...defaultAuthoredFlavor(options),
      values: filterTrackedSurfaceValues(metadata?.surface, options[0]?.values),
    };
    const selectedOptions = [{ ...selectedFlavor, selected: true }];
    const preservedMetadata = {
      ...metadata,
      authored_flavors: authoredFlavors,
      local_flavors: [],
      flavor_options: selectedOptions,
      flavors: selectedOptions.map((entry) => entry.name),
    };
    const nextMetadata = flattenCubeGroupMetadata(
      writeCubePresetMetadata(metadata, {
        flavor: 'default',
        flavor_scope: 'authored',
        active_flavor_values: cloneValue(selectedFlavor.values || {}),
      }),
      preservedMetadata,
    );
    setGroupSugarcubes(group, nextMetadata);
    if (
      selectedFlavor &&
      (forceApply || this.selectionNeedsApplication(metadata, selectedFlavor))
    ) {
      this.applyFlavorValues(graph, asFlavorMetadata(nextMetadata), selectedFlavor);
      this.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-refresh' });
    }
  }

  async reconcileGroupLocalFlavors(
    graph: ComfyGraph,
    group: ComfyGroup,
    metadata: FlavorMetadata,
  ): Promise<void> {
    const authoredFlavors = normalizeAuthoredFlavorEntries(
      metadata?.authored_flavors || metadata?.flavors,
      metadata?.surface,
    );
    if (!metadata?.cube_id || !metadata?.surface_signature || !authoredFlavors.length) {
      return;
    }
    const localSurfaceState = this.storage.readSurfaceState(
      metadata.cube_id,
      metadata.surface_signature,
    );
    const renameMap = await this.promptForLocalFlavorCollisionRenames({
      authoredFlavors,
      localFlavors: normalizeLocalFlavorEntries(localSurfaceState.flavors, metadata?.surface),
    });
    const result = await this.storage.reconcileLocalFlavors({
      cubeId: metadata.cube_id,
      surfaceSignature: metadata.surface_signature,
      authoredFlavors,
      renameMap,
    });
    if (!result?.conflict_count) {
      return;
    }
    const current = asFlavorMetadata(getGroupSugarcubes(group) || metadata);
    const currentSurface = current?.surface || metadata?.surface;
    const nextLocalFlavors = normalizeLocalFlavorEntries(
      this.storage.readSurfaceState(metadata.cube_id, metadata.surface_signature).flavors,
      currentSurface,
    );
    setGroupSugarcubes(group, {
      ...current,
      local_flavors: nextLocalFlavors,
    });
    this.toast?.push?.(
      'warn',
      'Local flavors renamed',
      `${result.conflict_count} local flavor name conflicted with authored flavors.`,
    );
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-reconcile' });
  }

  async promptForLocalFlavorCollisionRenames({
    authoredFlavors = [],
    localFlavors = [],
  }: {
    authoredFlavors?: FlavorOption[];
    localFlavors?: FlavorOption[];
  } = {}): Promise<Record<string, string>> {
    const collisions = findLocalFlavorCollisions(localFlavors, authoredFlavors);
    if (!collisions.length || !this.dialogs?.promptText) {
      return {};
    }
    const authoredKeys = flavorKeySets(authoredFlavors);
    const renameMap: Record<string, string> = {};
    const renamedNames = new Set<string>();
    for (const flavor of collisions) {
      const fallbackName = `${flavor?.name || flavor?.id || 'flavor'}_local`;
      const replacement =
        (await this.dialogs.promptText({
          title: 'Rename Local Flavor',
          message: [
            `"${flavor?.name || flavor?.id}" now conflicts with an authored flavor. Rename the local flavor to keep it.`,
          ],
          label: 'Local flavor name',
          initialValue: fallbackName,
          confirmLabel: 'Rename Flavor',
          normalizeValue: (value: unknown) => String(value).trim(),
        })) || '';
      const nameKey = normalizeFlavorNameKey(replacement);
      const idKey = normalizeFlavorId(replacement);
      if (
        !replacement ||
        authoredKeys.names.has(nameKey) ||
        authoredKeys.ids.has(idKey) ||
        renamedNames.has(nameKey)
      ) {
        continue;
      }
      renameMap[flavor.id] = replacement;
      renamedNames.add(nameKey);
    }
    return renameMap;
  }

  selectionNeedsApplication(
    metadata: FlavorMetadata,
    selectedFlavor: FlavorOption | null,
  ): boolean {
    if (!selectedFlavor) {
      return false;
    }
    if (metadata?.flavor !== selectedFlavor.id || metadata?.flavor_scope !== selectedFlavor.scope) {
      return true;
    }
    try {
      const activeValues = filterTrackedSurfaceValues(
        metadata?.surface,
        metadata?.active_flavor_values,
      );
      return JSON.stringify(activeValues || {}) !== JSON.stringify(selectedFlavor.values || {});
    } catch (_error) {
      return true;
    }
  }

  getGraph(): ComfyGraph | null {
    return this.adapter?.getApp?.()?.graph || null;
  }

  findGroupByMetadata(graph: ComfyGraph | null, metadata: FlavorMetadata): ComfyGroup | null {
    const instanceId = typeof metadata?.instance_id === 'string' ? metadata.instance_id.trim() : '';
    if (!graph || !instanceId) {
      return null;
    }
    return (
      getGraphGroups(graph).find((group) => {
        const entry = getGroupSugarcubes(group);
        return entry?.instance_id === instanceId;
      }) || null
    );
  }

  /**
   * Resolve live nodes that own the current cube surface controls.
   */
  buildNodesBySymbol(graph: ComfyGraph, metadata: FlavorMetadata): Map<string, ComfyNode> {
    return buildSurfaceNodesBySymbol(graph, metadata?.nodes, metadata?.surface);
  }

  collectCurrentSurfaceValues(graph: ComfyGraph, metadata: FlavorMetadata): UnknownRecord {
    const controls = trackedSurfaceControls(metadata?.surface);
    const nodesBySymbol = this.buildNodesBySymbol(graph, metadata);
    const values: UnknownRecord = {};
    for (const control of controls) {
      const controlId = typeof control?.control_id === 'string' ? control.control_id.trim() : '';
      const symbol = typeof control?.symbol === 'string' ? control.symbol.trim() : '';
      const inputName = typeof control?.input_name === 'string' ? control.input_name.trim() : '';
      if (!controlId || !symbol || !inputName) {
        continue;
      }
      const node = nodesBySymbol.get(symbol);
      if (!node) {
        continue;
      }
      const widgetValue = readWidgetValue(node, inputName);
      values[controlId] =
        widgetValue !== '' || readNodePropertyValue(node, inputName) == null
          ? cloneValue(widgetValue)
          : cloneValue(readNodePropertyValue(node, inputName));
    }
    return values;
  }

  applyFlavorValues(graph: ComfyGraph, metadata: FlavorMetadata, flavor: FlavorOption): void {
    const controls = trackedSurfaceControls(metadata?.surface);
    const nodesBySymbol = this.buildNodesBySymbol(graph, metadata);
    const trackedValues = filterTrackedSurfaceValues(metadata?.surface, flavor?.values);
    for (const control of controls) {
      const controlId = typeof control?.control_id === 'string' ? control.control_id.trim() : '';
      const symbol = typeof control?.symbol === 'string' ? control.symbol.trim() : '';
      const inputName = typeof control?.input_name === 'string' ? control.input_name.trim() : '';
      if (!controlId || !symbol || !inputName) {
        continue;
      }
      if (!Object.prototype.hasOwnProperty.call(trackedValues, controlId)) {
        continue;
      }
      const node = nodesBySymbol.get(symbol);
      if (!node) {
        continue;
      }
      applyNodeValue(node, inputName, cloneValue(trackedValues[controlId]));
    }
    const group = this.findGroupByMetadata(graph, metadata);
    if (group) {
      const current = asFlavorMetadata(getGroupSugarcubes(group));
      const flavorOptions = Array.isArray(current?.flavor_options)
        ? current.flavor_options.map((entry) => ({
            ...entry,
            selected: entry?.id === flavor.id && entry?.scope === flavor.scope,
          }))
        : [];
      setGroupSugarcubes(group, {
        ...current,
        flavor: flavor.id,
        flavor_scope: flavor.scope,
        flavor_options: flavorOptions,
        flavors: flavorOptions.map((entry) => entry.name),
        active_flavor_values: cloneValue(trackedValues),
      });
    }
    graph?.setDirtyCanvas?.(true, true);
    this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
  }

  async selectFlavor({
    metadata = {},
    flavor = null,
  }: {
    metadata?: FlavorMetadata;
    flavor?: FlavorOption | string | unknown;
  } = {}): Promise<void> {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !currentMetadata) {
      return;
    }
    const options = Array.isArray(currentMetadata?.flavor_options)
      ? currentMetadata.flavor_options
      : [];
    const nextFlavor =
      typeof flavor === 'string'
        ? options.find((entry) => entry?.id === flavor || entry?.name === flavor)
        : isRecord(flavor)
          ? (flavor as unknown as FlavorOption)
          : null;
    if (!nextFlavor || nextFlavor.stale) {
      return;
    }
    if (nextFlavor.id !== 'default' || nextFlavor.scope !== 'authored') {
      return;
    }
    this.applyFlavorValues(graph, currentMetadata, nextFlavor);
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-select' });
  }

  async saveCurrentFaceValuesAsDefault(metadata: FlavorMetadata): Promise<boolean> {
    return this.saveAuthoredFlavor(metadata, { flavorId: 'default', flavorName: 'Default' });
  }

  async saveCurrentFaceValuesAsCubeDefaults(metadata: FlavorMetadata): Promise<boolean> {
    return this.saveCurrentFaceValuesAsDefault(metadata);
  }

  async saveCurrentFaceValuesAsAuthoredFlavor(metadata: FlavorMetadata): Promise<boolean> {
    const flavorName =
      (await this.dialogs?.promptText?.({
        title: 'Save Authored Flavor',
        message: ['Name the authored flavor to save into the cube definition.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value: unknown) => String(value).trim(),
      })) || '';
    if (!flavorName) {
      return false;
    }
    return this.saveAuthoredFlavor(metadata, { flavorName });
  }

  async saveAuthoredFlavor(
    metadata: FlavorMetadata,
    { flavorId = '', flavorName = '' }: { flavorId?: string; flavorName?: string } = {},
  ): Promise<boolean> {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !currentMetadata?.cube_id) {
      return false;
    }
    const values = this.collectCurrentSurfaceValues(graph, currentMetadata);
    const payload = {
      cube_id: currentMetadata.cube_id,
      flavor_id: flavorId || '',
      flavor_name: flavorName || '',
      values,
    };
    if (!this.api?.saveAuthoredFlavor) {
      return false;
    }
    const { response, data } = await this.api.saveAuthoredFlavor(JSON.stringify(payload), {
      headers: { 'Content-Type': 'application/json' },
    });
    const isDefaultSave = (flavorId || '') === 'default';
    if (!response.ok || data?.error) {
      const fallback = isDefaultSave
        ? 'Current values could not be saved as cube defaults.'
        : 'Flavor save failed';
      const message = responseErrorMessage(data) || response.statusText || fallback;
      this.toast?.push?.(
        'error',
        isDefaultSave ? 'Default save failed' : 'Flavor save failed',
        message,
      );
      return false;
    }
    const resolvedFlavorId = savedFlavorId(data) || flavorId || 'default';
    const refreshedAuthored = normalizeAuthoredFlavorEntries(
      this.mergeAuthoredFlavorValues(currentMetadata.authored_flavors, {
        id: resolvedFlavorId,
        name: flavorName || (resolvedFlavorId === 'default' ? 'Default' : resolvedFlavorId),
        values,
      }),
      currentMetadata.surface,
    );
    const nextMetadata = {
      ...currentMetadata,
      authored_flavors: refreshedAuthored,
      flavor: resolvedFlavorId,
      flavor_scope: 'authored',
      active_flavor_values: cloneValue(values),
    };
    setGroupSugarcubes(group, nextMetadata);
    this.refreshGroupMetadata(graph, group, nextMetadata);
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'authored-flavor-save' });
    this.cubeBrowser?.refresh?.({ force: true }).catch(() => {});
    if (isDefaultSave) {
      this.toast?.push?.(
        'success',
        'Cube defaults saved',
        'Current values saved as cube defaults.',
      );
    }
    return true;
  }

  /** Prime the local flavor cache without exposing persistence internals. */
  async loadLocalFlavorState(cubeId: string): Promise<void> {
    await this.storage.loadCubeState(cubeId);
  }

  mergeAuthoredFlavorValues(
    existing: unknown,
    nextFlavor: { id?: string; name?: string; values?: UnknownRecord },
  ): FlavorOption[] {
    const authored = normalizeAuthoredFlavorEntries(existing);
    const flavorId = nextFlavor?.id || 'default';
    const index = authored.findIndex((entry) => entry.id === flavorId);
    const mergedEntry = {
      id: flavorId,
      name: nextFlavor?.name || (flavorId === 'default' ? 'Default' : flavorId),
      values: cloneValue(nextFlavor?.values || {}),
    };
    if (index === -1) {
      authored.push({ ...mergedEntry, scope: 'authored', stale: false });
      return authored;
    }
    authored[index] = {
      ...(authored[index] ?? { scope: 'authored', stale: false }),
      ...mergedEntry,
      scope: 'authored',
      stale: false,
    };
    return authored;
  }

  async saveCurrentFaceValuesAsLocalFlavor(metadata: FlavorMetadata): Promise<boolean> {
    const flavorName =
      (await this.dialogs?.promptText?.({
        title: 'Save Local Flavor',
        message: ['Name the local flavor to store for this surface.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value: unknown) => String(value).trim(),
      })) || '';
    if (!flavorName) {
      return false;
    }
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !currentMetadata?.cube_id || !currentMetadata?.surface_signature) {
      return false;
    }
    const values = this.collectCurrentSurfaceValues(graph, currentMetadata);
    let savedFlavor = null;
    try {
      savedFlavor = await this.storage.saveLocalFlavor({
        cubeId: currentMetadata.cube_id,
        surfaceSignature: currentMetadata.surface_signature,
        name: flavorName,
        values,
        ...(currentMetadata.authored_flavors
          ? { authoredFlavors: currentMetadata.authored_flavors }
          : {}),
      });
    } catch (error: unknown) {
      this.toast?.push?.(
        'error',
        'Local flavor save failed',
        errorMessage(error, 'Local flavor could not be saved.'),
      );
      return false;
    }
    this.refreshGroupMetadata(graph, group, {
      ...currentMetadata,
      flavor: typeof savedFlavor?.id === 'string' ? savedFlavor.id : currentMetadata.flavor || '',
      flavor_scope: 'local',
      active_flavor_values: cloneValue(values),
    });
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-save' });
    return true;
  }

  async deleteLocalFlavor(metadata: FlavorMetadata, flavorId: string): Promise<boolean> {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !currentMetadata?.cube_id || !currentMetadata?.surface_signature || !flavorId) {
      return false;
    }
    let deleted = false;
    try {
      deleted = await this.storage.deleteLocalFlavor({
        cubeId: currentMetadata.cube_id,
        surfaceSignature: currentMetadata.surface_signature,
        flavorId,
      });
    } catch (error: unknown) {
      this.toast?.push?.(
        'error',
        'Local flavor delete failed',
        errorMessage(error, 'Local flavor could not be deleted.'),
      );
      return false;
    }
    if (!deleted) {
      return false;
    }
    this.refreshGroupMetadata(graph, group, currentMetadata);
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-delete' });
    return true;
  }

  async manageFlavors(metadata: FlavorMetadata): Promise<boolean> {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    const localFlavors = Array.isArray(currentMetadata?.local_flavors)
      ? currentMetadata.local_flavors
      : [];
    if (!localFlavors.length) {
      this.toast?.push?.('info', 'No local flavors', 'There are no local flavors to manage.');
      return false;
    }
    const selected =
      (await this.dialogs?.selectItem?.({
        title: 'Delete Local Flavor',
        message: ['Choose the local flavor to remove from this surface.'],
        confirmLabel: 'Delete Flavor',
        items: localFlavors.map((entry) => ({
          value: entry.id,
          label: entry.name || entry.id,
          description: entry.id,
        })),
      })) || '';
    if (!selected) {
      return false;
    }
    const deleted = await this.deleteLocalFlavor(currentMetadata, selected);
    if (!deleted) {
      this.toast?.push?.('warn', 'Flavor not found', selected);
      return false;
    }
    this.toast?.push?.('success', 'Local flavor deleted', selected);
    return true;
  }
}
