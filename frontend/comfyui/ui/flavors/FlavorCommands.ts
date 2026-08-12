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
/** Own authored and local Flavor command workflows. */

import { getGroupSugarcubes, setGroupSugarcubes } from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
import {
  asFlavorMetadata,
  cloneValue,
  errorMessage,
  normalizeAuthoredFlavorEntries,
  resolveFlavorNameSeed,
  responseErrorMessage,
  savedFlavorId,
  type FlavorApi,
  type FlavorBrowser,
  type FlavorDialogs,
  type FlavorDirtyManager,
  type FlavorMetadata,
  type FlavorToast,
} from './FlavorSupport.js';
import type { FlavorStorage } from './FlavorStorage.js';
import type { FlavorGraphProjection } from './FlavorGraphProjection.js';
import type { FlavorDefinitionLifecycle } from './FlavorDefinitionLifecycle.js';
import type { FlavorOption } from './FlavorSelection.js';
import type { UnknownRecord } from '../types/common.js';

interface CommandOptions {
  dialogs: FlavorDialogs | null;
  toast: FlavorToast | null;
  api: FlavorApi | null;
  dirtyManager: FlavorDirtyManager | null;
  cubeBrowser: FlavorBrowser | null;
  storage: FlavorStorage;
  graph: FlavorGraphProjection;
  lifecycle: FlavorDefinitionLifecycle;
}

/** Execute Flavor selection, persistence, and management commands. */
export class FlavorCommands {
  constructor(private readonly options: CommandOptions) {}

  async select(metadata: FlavorMetadata, flavor: FlavorOption | string | unknown): Promise<void> {
    const graph = this.options.graph.getGraph();
    const group = this.options.graph.findGroup(graph, metadata);
    const current = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !current) return;
    const choices = current.flavor_options || [];
    const next =
      typeof flavor === 'string'
        ? choices.find((entry) => entry.id === flavor || entry.name === flavor)
        : isRecord(flavor)
          ? (flavor as unknown as FlavorOption)
          : null;
    if (!next || next.stale || next.id !== 'default' || next.scope !== 'authored') return;
    this.options.graph.applyValues(graph, current, next);
    this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'flavor-select' });
  }

  async promptAndSaveAuthored(metadata: FlavorMetadata): Promise<boolean> {
    const name =
      (await this.options.dialogs?.promptText?.({
        title: 'Save Authored Flavor',
        message: ['Name the authored flavor to save into the cube definition.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value: unknown) => String(value).trim(),
      })) || '';
    return name ? this.saveAuthored(metadata, { flavorName: name }) : false;
  }

  async saveAuthored(
    metadata: FlavorMetadata,
    { flavorId = '', flavorName = '' }: { flavorId?: string; flavorName?: string } = {},
  ): Promise<boolean> {
    const graph = this.options.graph.getGraph();
    const group = this.options.graph.findGroup(graph, metadata);
    const current = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !current.cube_id || !this.options.api?.saveAuthoredFlavor) return false;
    const values = this.options.graph.collectValues(graph, current);
    const { response, data } = await this.options.api.saveAuthoredFlavor(
      JSON.stringify({
        cube_id: current.cube_id,
        flavor_id: flavorId,
        flavor_name: flavorName,
        values,
      }),
      { headers: { 'Content-Type': 'application/json' } },
    );
    const isDefault = flavorId === 'default';
    if (!response.ok || data.error) {
      const fallback = isDefault
        ? 'Current values could not be saved as cube defaults.'
        : 'Flavor save failed';
      this.options.toast?.push?.(
        'error',
        isDefault ? 'Default save failed' : 'Flavor save failed',
        responseErrorMessage(data) || response.statusText || fallback,
      );
      return false;
    }
    const resolvedId = savedFlavorId(data) || flavorId || 'default';
    const authored = normalizeAuthoredFlavorEntries(
      this.mergeAuthoredValues(current.authored_flavors, {
        id: resolvedId,
        name: flavorName || (resolvedId === 'default' ? 'Default' : resolvedId),
        values,
      }),
      current.surface,
    );
    const next = {
      ...current,
      authored_flavors: authored,
      flavor: resolvedId,
      flavor_scope: 'authored',
      active_flavor_values: cloneValue(values),
    };
    setGroupSugarcubes(group, next);
    this.options.lifecycle.refreshGroup(graph, group, next);
    this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'authored-flavor-save' });
    void this.options.cubeBrowser?.refresh?.({ force: true }).catch(() => {});
    if (isDefault)
      this.options.toast?.push?.(
        'success',
        'Cube defaults saved',
        'Current values saved as cube defaults.',
      );
    return true;
  }

  mergeAuthoredValues(
    existing: unknown,
    nextFlavor: { id?: string; name?: string; values?: UnknownRecord },
  ): FlavorOption[] {
    const authored = normalizeAuthoredFlavorEntries(existing);
    const id = nextFlavor.id || 'default';
    const index = authored.findIndex((entry) => entry.id === id);
    const merged = {
      id,
      name: nextFlavor.name || (id === 'default' ? 'Default' : id),
      values: cloneValue(nextFlavor.values || {}),
      scope: 'authored' as const,
      stale: false,
    };
    if (index === -1) authored.push(merged);
    else authored[index] = { ...authored[index], ...merged };
    return authored;
  }

  async promptAndSaveLocal(metadata: FlavorMetadata): Promise<boolean> {
    const name =
      (await this.options.dialogs?.promptText?.({
        title: 'Save Local Flavor',
        message: ['Name the local flavor to store for this surface.'],
        label: 'Flavor name',
        initialValue: resolveFlavorNameSeed(metadata),
        confirmLabel: 'Save Flavor',
        normalizeValue: (value: unknown) => String(value).trim(),
      })) || '';
    if (!name) return false;
    const graph = this.options.graph.getGraph();
    const group = this.options.graph.findGroup(graph, metadata);
    const current = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !current.cube_id || !current.surface_signature) return false;
    const values = this.options.graph.collectValues(graph, current);
    try {
      const saved = await this.options.storage.saveLocalFlavor({
        cubeId: current.cube_id,
        surfaceSignature: current.surface_signature,
        name,
        values,
        ...(current.authored_flavors ? { authoredFlavors: current.authored_flavors } : {}),
      });
      this.options.lifecycle.refreshGroup(graph, group, {
        ...current,
        flavor: typeof saved?.id === 'string' ? saved.id : current.flavor || '',
        flavor_scope: 'local',
        active_flavor_values: cloneValue(values),
      });
      this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-save' });
      return true;
    } catch (error) {
      this.options.toast?.push?.(
        'error',
        'Local flavor save failed',
        errorMessage(error, 'Local flavor could not be saved.'),
      );
      return false;
    }
  }

  async deleteLocal(metadata: FlavorMetadata, flavorId: string): Promise<boolean> {
    const graph = this.options.graph.getGraph();
    const group = this.options.graph.findGroup(graph, metadata);
    const current = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    if (!graph || !current.cube_id || !current.surface_signature || !flavorId) return false;
    try {
      const deleted = await this.options.storage.deleteLocalFlavor({
        cubeId: current.cube_id,
        surfaceSignature: current.surface_signature,
        flavorId,
      });
      if (!deleted) return false;
      this.options.lifecycle.refreshGroup(graph, group, current);
      this.options.dirtyManager?.requestRefresh?.({ graph, reason: 'local-flavor-delete' });
      return true;
    } catch (error) {
      this.options.toast?.push?.(
        'error',
        'Local flavor delete failed',
        errorMessage(error, 'Local flavor could not be deleted.'),
      );
      return false;
    }
  }

  async manage(metadata: FlavorMetadata): Promise<boolean> {
    const graph = this.options.graph.getGraph();
    const group = this.options.graph.findGroup(graph, metadata);
    const current = group ? asFlavorMetadata(getGroupSugarcubes(group)) : metadata;
    const local = current.local_flavors || [];
    if (!local.length) {
      this.options.toast?.push?.(
        'info',
        'No local flavors',
        'There are no local flavors to manage.',
      );
      return false;
    }
    const selected =
      (await this.options.dialogs?.selectItem?.({
        title: 'Delete Local Flavor',
        message: ['Choose the local flavor to remove from this surface.'],
        confirmLabel: 'Delete Flavor',
        items: local.map((entry) => ({
          value: entry.id,
          label: entry.name || entry.id,
          description: entry.id,
        })),
      })) || '';
    if (!selected) return false;
    if (!(await this.deleteLocal(current, selected))) {
      this.options.toast?.push?.('warn', 'Flavor not found', selected);
      return false;
    }
    this.options.toast?.push?.('success', 'Local flavor deleted', selected);
    return true;
  }
}
