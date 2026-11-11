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
/** Own Flavor definition hydration and managed-group metadata refresh. */

import { buildCubeDefinitionKey } from '../core/CubeDefinitionKey.js';
import { filterTrackedSurfaceValues } from '../core/SurfaceValuePolicy.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import {
  flattenCubeGroupMetadata,
  getGroupSugarcubes,
  setGroupSugarcubes,
  writeCubeDefinitionMetadata,
  writeCubePresetMetadata,
} from '../graph/GroupMetadata.js';
import { defaultAuthoredFlavor, defaultsOnlyFlavorOptions } from './FlavorSelection.js';
import {
  asFlavorMetadata,
  cloneValue,
  findLocalFlavorCollisions,
  flavorKeySets,
  normalizeAuthoredFlavorEntries,
  normalizeFlavorNameKey,
  normalizeLocalFlavorEntries,
  normalizeSurface,
  type CubeFlavorPayload,
  type FlavorDialogs,
  type FlavorDirtyManager,
  type FlavorMetadata,
  type FlavorToast,
  type HydrateOptions,
  type ImportedFlavorMetadata,
} from './FlavorSupport.js';
import { normalizeFlavorId, type FlavorOption } from './FlavorSelection.js';
import type { FlavorStorage } from './FlavorStorage.js';
import type { FlavorGraphProjection } from './FlavorGraphProjection.js';
import type { ComfyGraph, ComfyGroup } from '../types/graph.js';
import { isRecord } from '../types/common.js';

interface LifecycleOptions {
  dialogs: FlavorDialogs | null;
  toast: FlavorToast | null;
  dirtyManager: FlavorDirtyManager | null;
  storage: FlavorStorage;
  graph: FlavorGraphProjection;
}

/** Hydrate definition defaults and reconcile their graph metadata projection. */
export class FlavorDefinitionLifecycle {
  constructor(private readonly options: LifecycleOptions) {}

  buildImportedMetadata(cube: CubeFlavorPayload): ImportedFlavorMetadata {
    const surface = normalizeSurface(cube.surface);
    const authored = normalizeAuthoredFlavorEntries(cube.flavors?.authored, surface);
    const selected = defaultAuthoredFlavor(authored);
    const values = filterTrackedSurfaceValues(surface, selected.values);
    const defaultOption = { ...selected, values: cloneValue(values) };
    return {
      surface,
      surface_signature: typeof cube.surface_signature === 'string' ? cube.surface_signature : '',
      authored_flavors: authored,
      flavor: 'default',
      flavor_scope: 'authored',
      active_flavor_values: cloneValue(values),
      flavor_options: [defaultOption],
      flavors: [defaultOption.name],
      local_flavors: [],
    };
  }

  async hydrate({
    cubeId,
    definitionKey,
    entry,
    graph,
    forceApply = false,
  }: HydrateOptions = {}): Promise<number> {
    const entryRecord = isRecord(entry) ? entry : {};
    const payload = isRecord(entryRecord.payload) ? entryRecord.payload : {};
    const cube = isRecord(payload.cube) ? (payload.cube as CubeFlavorPayload) : null;
    if (!graph || !cubeId || !cube) return 0;
    const targetKey = definitionKey?.trim() || buildCubeDefinitionKey(cubeId, cube.version);
    const imported = this.buildImportedMetadata(cube);
    let count = 0;
    for (const group of getGraphGroups(graph)) {
      const metadata = asFlavorMetadata(getGroupSugarcubes(group));
      if (!metadata.managed || metadata.cube_id !== cubeId) continue;
      const hasVersion = Boolean(metadata.cube_definition_key || metadata.cube_version);
      const groupKey =
        metadata.cube_definition_key?.trim() ||
        buildCubeDefinitionKey(metadata.cube_id, metadata.cube_version);
      if (hasVersion && groupKey && targetKey && groupKey !== targetKey) continue;
      const definition = writeCubeDefinitionMetadata(metadata, {
        surface: imported.surface,
        surface_signature: imported.surface_signature,
      });
      const preset = writeCubePresetMetadata(definition, {
        flavor: imported.flavor,
        flavor_scope: imported.flavor_scope,
        active_flavor_values: imported.active_flavor_values,
      });
      setGroupSugarcubes(
        group,
        flattenCubeGroupMetadata(preset, {
          ...metadata,
          authored_flavors: imported.authored_flavors,
          flavor_options: imported.flavor_options,
          flavors: imported.flavors,
          local_flavors: imported.local_flavors,
        }),
      );
      this.refreshGroup(graph, group, asFlavorMetadata(getGroupSugarcubes(group)), { forceApply });
      count += 1;
    }
    return count;
  }

  refreshGraph(graph: ComfyGraph | null): void {
    if (!graph) return;
    for (const group of getGraphGroups(graph)) {
      const metadata = asFlavorMetadata(getGroupSugarcubes(group));
      if (!metadata.managed || !metadata.instance_id || !metadata.cube_id) continue;
      if (!Array.isArray(metadata.authored_flavors) && !Array.isArray(metadata.flavors)) continue;
      this.refreshGroup(graph, group, metadata);
    }
  }

  refreshGroup(
    graph: ComfyGraph,
    group: ComfyGroup | null,
    metadata: FlavorMetadata,
    { forceApply = false }: { forceApply?: boolean } = {},
  ): void {
    const authored = normalizeAuthoredFlavorEntries(
      metadata.authored_flavors || metadata.flavors,
      metadata.surface,
    );
    const options = defaultsOnlyFlavorOptions(authored);
    const selected: FlavorOption = {
      ...defaultAuthoredFlavor(options),
      values: filterTrackedSurfaceValues(metadata.surface, options[0]?.values),
    };
    const selectedOptions = [{ ...selected, selected: true }];
    const next = flattenCubeGroupMetadata(
      writeCubePresetMetadata(metadata, {
        flavor: 'default',
        flavor_scope: 'authored',
        active_flavor_values: cloneValue(selected.values || {}),
      }),
      {
        ...metadata,
        authored_flavors: authored,
        local_flavors: [],
        flavor_options: selectedOptions,
        flavors: selectedOptions.map((entry) => entry.name),
      },
    );
    setGroupSugarcubes(group, next);
    if (forceApply || this.options.graph.selectionNeedsApplication(metadata, selected)) {
      this.options.graph.applyValues(graph, asFlavorMetadata(next), selected);
      this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-refresh' });
    }
  }

  async reconcileLocalFlavors(
    graph: ComfyGraph,
    group: ComfyGroup,
    metadata: FlavorMetadata,
  ): Promise<void> {
    const authored = normalizeAuthoredFlavorEntries(
      metadata.authored_flavors || metadata.flavors,
      metadata.surface,
    );
    if (!metadata.cube_id || !metadata.surface_signature || !authored.length) return;
    const local = this.options.storage.readSurfaceState(
      metadata.cube_id,
      metadata.surface_signature,
    );
    const renameMap = await this.promptCollisionRenames({
      authoredFlavors: authored,
      localFlavors: normalizeLocalFlavorEntries(local.flavors, metadata.surface),
    });
    const result = await this.options.storage.reconcileLocalFlavors({
      cubeId: metadata.cube_id,
      surfaceSignature: metadata.surface_signature,
      authoredFlavors: authored,
      renameMap,
    });
    if (!result.conflict_count) return;
    const current = asFlavorMetadata(getGroupSugarcubes(group) || metadata);
    const nextLocal = normalizeLocalFlavorEntries(
      this.options.storage.readSurfaceState(metadata.cube_id, metadata.surface_signature).flavors,
      current.surface || metadata.surface,
    );
    setGroupSugarcubes(group, { ...current, local_flavors: nextLocal });
    this.options.toast?.push?.(
      'warn',
      'Local flavors renamed',
      `${result.conflict_count} local flavor name conflicted with authored flavors.`,
    );
    this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-reconcile' });
  }

  async promptCollisionRenames({
    authoredFlavors = [],
    localFlavors = [],
  }: { authoredFlavors?: FlavorOption[]; localFlavors?: FlavorOption[] } = {}): Promise<
    Record<string, string>
  > {
    const collisions = findLocalFlavorCollisions(localFlavors, authoredFlavors);
    if (!collisions.length || !this.options.dialogs?.promptText) return {};
    const authoredKeys = flavorKeySets(authoredFlavors);
    const renameMap: Record<string, string> = {};
    const renamedNames = new Set<string>();
    for (const flavor of collisions) {
      const replacement =
        (await this.options.dialogs.promptText({
          title: 'Rename Local Flavor',
          message: [
            `"${flavor.name || flavor.id}" now conflicts with an authored flavor. Rename the local flavor to keep it.`,
          ],
          label: 'Local flavor name',
          initialValue: `${flavor.name || flavor.id || 'flavor'}_local`,
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
      )
        continue;
      renameMap[flavor.id] = replacement;
      renamedNames.add(nameKey);
    }
    return renameMap;
  }
}
