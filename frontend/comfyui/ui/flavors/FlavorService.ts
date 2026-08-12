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
/** Compose Flavor event, definition, graph, and command owners. */

import { isRecord } from '../types/common.js';
import { FlavorCommands } from './FlavorCommands.js';
import { FlavorDefinitionLifecycle } from './FlavorDefinitionLifecycle.js';
import { FlavorGraphProjection } from './FlavorGraphProjection.js';
import { FlavorStorage } from './FlavorStorage.js';
import {
  asFlavorMetadata,
  errorMessage,
  type CubeFlavorPayload,
  type FlavorMetadata,
  type FlavorServiceOptions,
  type HydrateOptions,
  type ImportedFlavorMetadata,
} from './FlavorSupport.js';
import type { FlavorOption } from './FlavorSelection.js';
import type { ComfyGraph, ComfyGroup, ComfyNode } from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';

export type { CubeFlavorPayload, FlavorApi, FlavorMetadata } from './FlavorSupport.js';

/** Coordinate the public Flavor API while focused collaborators own behavior. */
export class FlavorService {
  private readonly events: FlavorServiceOptions['events'];
  private readonly toast: FlavorServiceOptions['toast'];
  private readonly storage: FlavorStorage;
  private readonly graph: FlavorGraphProjection;
  private readonly lifecycle: FlavorDefinitionLifecycle;
  private readonly commands: FlavorCommands;
  private readonly unsubscribers: Array<() => void> = [];

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
    this.events = events || null;
    this.toast = toast || null;
    this.storage = new FlavorStorage({
      ...(storage !== undefined ? { storage } : {}),
      ...(api !== undefined ? { api } : {}),
    });
    this.graph = new FlavorGraphProjection(adapter || null);
    this.lifecycle = new FlavorDefinitionLifecycle({
      dialogs: dialogs || null,
      toast: toast || null,
      dirtyManager: dirtyManager || null,
      storage: this.storage,
      graph: this.graph,
    });
    this.commands = new FlavorCommands({
      dialogs: dialogs || null,
      toast: toast || null,
      api: api || null,
      dirtyManager: dirtyManager || null,
      cubeBrowser: cubeBrowser || null,
      storage: this.storage,
      graph: this.graph,
      lifecycle: this.lifecycle,
    });
  }

  async setup(): Promise<void> {
    if (!this.events?.on) return;
    this.unsubscribers.push(
      this.events.on('cube:instances:updated', (payload) => {
        this.refreshGraph(isRecord(payload.graph) ? (payload.graph as ComfyGraph) : null);
      }),
      this.events.on('cube:flavor:change', (payload) => {
        void this.selectFlavor({
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
      this.events.on('cube:definition:loaded', (payload) => {
        const entry = isRecord(payload.entry) ? payload.entry : undefined;
        const graph = isRecord(payload.graph) ? (payload.graph as ComfyGraph) : undefined;
        void this.hydrateFromDefinition({
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
        /* Listener cleanup is best-effort. */
      }
    }
  }

  buildImportedMetadata(cube: CubeFlavorPayload): ImportedFlavorMetadata {
    return this.lifecycle.buildImportedMetadata(cube);
  }

  async hydrateFromDefinition(options: HydrateOptions = {}): Promise<number> {
    return this.lifecycle.hydrate(options);
  }

  refreshGraph(graph: ComfyGraph | null): void {
    this.lifecycle.refreshGraph(graph);
  }

  refreshGroupMetadata(
    graph: ComfyGraph,
    group: ComfyGroup | null,
    metadata: FlavorMetadata,
    options: { forceApply?: boolean } = {},
  ): void {
    this.lifecycle.refreshGroup(graph, group, metadata, options);
  }

  async reconcileGroupLocalFlavors(
    graph: ComfyGraph,
    group: ComfyGroup,
    metadata: FlavorMetadata,
  ): Promise<void> {
    await this.lifecycle.reconcileLocalFlavors(graph, group, metadata);
  }

  async promptForLocalFlavorCollisionRenames(
    options: { authoredFlavors?: FlavorOption[]; localFlavors?: FlavorOption[] } = {},
  ): Promise<Record<string, string>> {
    return this.lifecycle.promptCollisionRenames(options);
  }

  selectionNeedsApplication(metadata: FlavorMetadata, flavor: FlavorOption | null): boolean {
    return this.graph.selectionNeedsApplication(metadata, flavor);
  }

  getGraph(): ComfyGraph | null {
    return this.graph.getGraph();
  }

  findGroupByMetadata(graph: ComfyGraph | null, metadata: FlavorMetadata): ComfyGroup | null {
    return this.graph.findGroup(graph, metadata);
  }

  buildNodesBySymbol(graph: ComfyGraph, metadata: FlavorMetadata): Map<string, ComfyNode> {
    return this.graph.buildNodesBySymbol(graph, metadata);
  }

  collectCurrentSurfaceValues(graph: ComfyGraph, metadata: FlavorMetadata): UnknownRecord {
    return this.graph.collectValues(graph, metadata);
  }

  applyFlavorValues(graph: ComfyGraph, metadata: FlavorMetadata, flavor: FlavorOption): void {
    this.graph.applyValues(graph, metadata, flavor);
  }

  async selectFlavor({
    metadata = {},
    flavor = null,
  }: { metadata?: FlavorMetadata; flavor?: FlavorOption | string | unknown } = {}): Promise<void> {
    await this.commands.select(metadata, flavor);
  }

  async saveCurrentFaceValuesAsAuthoredFlavor(metadata: FlavorMetadata): Promise<boolean> {
    return this.commands.promptAndSaveAuthored(metadata);
  }

  async saveAuthoredFlavor(
    metadata: FlavorMetadata,
    options: { flavorId?: string; flavorName?: string } = {},
  ): Promise<boolean> {
    return this.commands.saveAuthored(metadata, options);
  }

  async loadLocalFlavorState(cubeId: string): Promise<void> {
    await this.storage.loadCubeState(cubeId);
  }

  mergeAuthoredFlavorValues(
    existing: unknown,
    next: { id?: string; name?: string; values?: UnknownRecord },
  ): FlavorOption[] {
    return this.commands.mergeAuthoredValues(existing, next);
  }

  async saveCurrentFaceValuesAsLocalFlavor(metadata: FlavorMetadata): Promise<boolean> {
    return this.commands.promptAndSaveLocal(metadata);
  }

  async deleteLocalFlavor(metadata: FlavorMetadata, flavorId: string): Promise<boolean> {
    return this.commands.deleteLocal(metadata, flavorId);
  }

  async manageFlavors(metadata: FlavorMetadata): Promise<boolean> {
    return this.commands.manage(metadata);
  }
}
