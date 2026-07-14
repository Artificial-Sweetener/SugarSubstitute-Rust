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
 * `web/comfyui/ui/flavors/FlavorService.js`.
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

function cloneValue(value) {
  return JSON.parse(JSON.stringify(value));
}

function readNodePropertyValue(node, inputName) {
  if (node?.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName)) {
    return node.properties[inputName];
  }
  return null;
}

function applyNodeValue(node, inputName, value) {
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

function normalizeSurface(surface) {
  if (!surface || typeof surface !== 'object') {
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

function normalizeAuthoredFlavorEntries(flavors, surface) {
  const normalized = normalizeAuthoredFlavors(flavors, surface?.default_flavor_id || 'default');
  return normalized.map((entry) => ({
    ...entry,
    values: filterTrackedSurfaceValues(surface, entry.values),
  }));
}

function normalizeLocalFlavorEntries(flavors, surface) {
  return (Array.isArray(flavors) ? flavors : []).map((entry) => ({
    ...entry,
    values: filterTrackedSurfaceValues(surface, entry?.values),
  }));
}

function resolveFlavorNameSeed(metadata) {
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

function normalizeFlavorNameKey(value) {
  return String(value || '')
    .trim()
    .toLowerCase();
}

function flavorKeySets(flavors) {
  const ids = new Set();
  const names = new Set();
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

function findLocalFlavorCollisions(localFlavors, authoredFlavors) {
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
  constructor({ adapter, dialogs, events, storage, toast, api, dirtyManager, cubeBrowser } = {}) {
    this.adapter = adapter;
    this.dialogs = dialogs || null;
    this.events = events;
    this.toast = toast || null;
    this.api = api || null;
    this.dirtyManager = dirtyManager || null;
    this.cubeBrowser = cubeBrowser || null;
    this.storage = new FlavorStorage({ storage, api });
    this.unsubscribers = [];
  }

  async setup() {
    if (!this.events?.on) {
      return;
    }
    this.unsubscribers.push(
      this.events.on('cube:instances:updated', ({ graph }) => {
        this.refreshGraph(graph);
      }),
    );
    this.unsubscribers.push(
      this.events.on('cube:flavor:change', ({ metadata, flavor }) => {
        this.selectFlavor({ metadata, flavor }).catch((error) => {
          this.toast?.push?.(
            'error',
            'Flavor selection failed',
            error?.message || 'Local flavor selection could not be saved.',
          );
        });
      }),
    );
    this.unsubscribers.push(
      this.events.on('cube:definition:loaded', ({ cubeId, definitionKey, entry, graph }) => {
        this.hydrateFromDefinition({ cubeId, definitionKey, entry, graph }).catch((error) => {
          this.toast?.push?.(
            'warn',
            'Cube defaults unavailable',
            error?.message || 'Cube default state could not be loaded.',
          );
        });
      }),
    );
  }

  dispose() {
    for (const unsubscribe of this.unsubscribers.splice(0)) {
      try {
        unsubscribe?.();
      } catch (_error) {
        // ignore listener cleanup failures
      }
    }
  }

  buildImportedMetadata(cube) {
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

  async hydrateFromDefinition({ cubeId, definitionKey, entry, graph, forceApply = false } = {}) {
    const definitionCube = entry?.payload?.cube;
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
      const metadata = getGroupSugarcubes(group);
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
      this.refreshGroupMetadata(graph, group, getGroupSugarcubes(group), { forceApply });
      hydratedCount += 1;
    }
    return hydratedCount;
  }

  refreshGraph(graph) {
    if (!graph) {
      return;
    }
    for (const group of getGraphGroups(graph)) {
      const metadata = getGroupSugarcubes(group);
      if (!metadata?.managed || !metadata?.instance_id || !metadata?.cube_id) {
        continue;
      }
      if (!Array.isArray(metadata?.authored_flavors) && !Array.isArray(metadata?.flavors)) {
        continue;
      }
      this.refreshGroupMetadata(graph, group, metadata);
    }
  }

  refreshGroupMetadata(graph, group, metadata, { forceApply = false } = {}) {
    const authoredFlavors = normalizeAuthoredFlavorEntries(
      metadata?.authored_flavors || metadata?.flavors,
      metadata?.surface,
    );
    const options = defaultsOnlyFlavorOptions(authoredFlavors);
    const selectedFlavor = {
      ...options[0],
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
      this.applyFlavorValues(graph, nextMetadata, selectedFlavor);
      this.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-refresh' });
    }
  }

  async reconcileGroupLocalFlavors(graph, group, metadata) {
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
    const current = getGroupSugarcubes(group) || metadata;
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

  async promptForLocalFlavorCollisionRenames({ authoredFlavors, localFlavors } = {}) {
    const collisions = findLocalFlavorCollisions(localFlavors, authoredFlavors);
    if (!collisions.length || !this.dialogs?.promptText) {
      return {};
    }
    const authoredKeys = flavorKeySets(authoredFlavors);
    const renameMap = {};
    const renamedNames = new Set();
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
          normalizeValue: (value) => value.trim(),
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

  selectionNeedsApplication(metadata, selectedFlavor) {
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

  getGraph() {
    return this.adapter?.getApp?.()?.graph || null;
  }

  findGroupByMetadata(graph, metadata) {
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
  buildNodesBySymbol(graph, metadata) {
    return buildSurfaceNodesBySymbol(graph, metadata?.nodes, metadata?.surface);
  }

  collectCurrentSurfaceValues(graph, metadata) {
    const controls = trackedSurfaceControls(metadata?.surface);
    const nodesBySymbol = this.buildNodesBySymbol(graph, metadata);
    const values = {};
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

  applyFlavorValues(graph, metadata, flavor) {
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
      const current = getGroupSugarcubes(group);
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

  async selectFlavor({ metadata, flavor } = {}) {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? getGroupSugarcubes(group) : metadata;
    if (!graph || !currentMetadata) {
      return;
    }
    const options = Array.isArray(currentMetadata?.flavor_options)
      ? currentMetadata.flavor_options
      : [];
    const nextFlavor =
      typeof flavor === 'string'
        ? options.find((entry) => entry?.id === flavor || entry?.name === flavor)
        : flavor;
    if (!nextFlavor || nextFlavor.stale) {
      return;
    }
    if (nextFlavor.id !== 'default' || nextFlavor.scope !== 'authored') {
      return;
    }
    this.applyFlavorValues(graph, currentMetadata, nextFlavor);
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-select' });
  }

  async saveCurrentFaceValuesAsDefault(metadata) {
    return this.saveAuthoredFlavor(metadata, { flavorId: 'default', flavorName: 'Default' });
  }

  async saveCurrentFaceValuesAsCubeDefaults(metadata) {
    return this.saveCurrentFaceValuesAsDefault(metadata);
  }

  async saveCurrentFaceValuesAsAuthoredFlavor(metadata) {
    const flavorName =
      (await this.dialogs?.promptText?.({
        title: 'Save Authored Flavor',
        message: ['Name the authored flavor to save into the cube definition.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value) => value.trim(),
      })) || '';
    if (!flavorName) {
      return false;
    }
    return this.saveAuthoredFlavor(metadata, { flavorName });
  }

  async saveAuthoredFlavor(metadata, { flavorId = '', flavorName = '' } = {}) {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? getGroupSugarcubes(group) : metadata;
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
    const { response, data } = await this.api.saveAuthoredFlavor(JSON.stringify(payload), {
      headers: { 'Content-Type': 'application/json' },
    });
    const isDefaultSave = (flavorId || '') === 'default';
    if (!response.ok || data?.error) {
      const fallback = isDefaultSave
        ? 'Current values could not be saved as cube defaults.'
        : 'Flavor save failed';
      const message = data?.error?.message || response.statusText || fallback;
      this.toast?.push?.(
        'error',
        isDefaultSave ? 'Default save failed' : 'Flavor save failed',
        message,
      );
      return false;
    }
    const savedFlavorId = data?.saved?.flavor_id || flavorId || 'default';
    const refreshedAuthored = normalizeAuthoredFlavorEntries(
      this.mergeAuthoredFlavorValues(currentMetadata.authored_flavors, {
        id: savedFlavorId,
        name: flavorName || (savedFlavorId === 'default' ? 'Default' : savedFlavorId),
        values,
      }),
      currentMetadata.surface,
    );
    const nextMetadata = {
      ...currentMetadata,
      authored_flavors: refreshedAuthored,
      flavor: savedFlavorId,
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

  mergeAuthoredFlavorValues(existing, nextFlavor) {
    const authored = normalizeAuthoredFlavorEntries(existing);
    const flavorId = nextFlavor?.id || 'default';
    const index = authored.findIndex((entry) => entry.id === flavorId);
    const mergedEntry = {
      id: flavorId,
      name: nextFlavor?.name || (flavorId === 'default' ? 'Default' : flavorId),
      values: cloneValue(nextFlavor?.values || {}),
    };
    if (index === -1) {
      authored.push(mergedEntry);
      return authored;
    }
    authored[index] = {
      ...authored[index],
      ...mergedEntry,
    };
    return authored;
  }

  async saveCurrentFaceValuesAsLocalFlavor(metadata) {
    const flavorName =
      (await this.dialogs?.promptText?.({
        title: 'Save Local Flavor',
        message: ['Name the local flavor to store for this surface.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value) => value.trim(),
      })) || '';
    if (!flavorName) {
      return false;
    }
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? getGroupSugarcubes(group) : metadata;
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
        authoredFlavors: currentMetadata.authored_flavors,
      });
    } catch (error) {
      this.toast?.push?.(
        'error',
        'Local flavor save failed',
        error?.message || 'Local flavor could not be saved.',
      );
      return false;
    }
    this.refreshGroupMetadata(graph, group, {
      ...currentMetadata,
      flavor: savedFlavor?.id || currentMetadata.flavor,
      flavor_scope: 'local',
      active_flavor_values: cloneValue(values),
    });
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-save' });
    return true;
  }

  async deleteLocalFlavor(metadata, flavorId) {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? getGroupSugarcubes(group) : metadata;
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
    } catch (error) {
      this.toast?.push?.(
        'error',
        'Local flavor delete failed',
        error?.message || 'Local flavor could not be deleted.',
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

  async manageFlavors(metadata) {
    const graph = this.getGraph();
    const group = this.findGroupByMetadata(graph, metadata);
    const currentMetadata = group ? getGroupSugarcubes(group) : metadata;
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
