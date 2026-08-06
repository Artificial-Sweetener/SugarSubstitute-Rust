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
/** Adapt picker definitions to Comfy's registration and reload lifecycle. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { CubePickerCatalogRegistry } from './CubePickerCatalogRegistry.js';
import type { ComfyCubeNodeDefinition } from './ComfyCubeNodeDefProjector.js';
import { isComfyCubePickerType } from './ComfyCubeNodeDefProjector.js';

export interface CubePickerDefinitionHost {
  registerNodeDef?(type: string, definition: UnknownRecord): unknown | Promise<unknown>;
  reloadNodeDefs?(): unknown | Promise<unknown>;
}

export interface CubePickerDefinitionLiteGraph {
  unregisterNodeType?(type: string): void;
}

export interface ComfyCubePickerDefinitionAdapterOptions {
  registry: CubePickerCatalogRegistry;
  app: CubePickerDefinitionHost;
  liteGraph: CubePickerDefinitionLiteGraph;
}

/** Own definition contribution, ordering, registration, and stale-type removal. */
export class ComfyCubePickerDefinitionAdapter {
  readonly #registry: CubePickerCatalogRegistry;
  readonly #app: CubePickerDefinitionHost;
  readonly #liteGraph: CubePickerDefinitionLiteGraph;
  #registeredTypes = new Set<string>();

  /** Bind catalog state to the narrow current Comfy definition capabilities. */
  constructor(options: ComfyCubePickerDefinitionAdapterOptions) {
    this.#registry = options.registry;
    this.#app = options.app;
    this.#liteGraph = options.liteGraph;
  }

  /** Contribute placement-ready definitions through Comfy's official startup hook. */
  async contribute(definitions: Record<string, UnknownRecord>): Promise<void> {
    await this.#registry.refresh();
    const projected = this.#registry.definitions();
    for (const definition of projected) definitions[definition.name] = toHostDefinition(definition);
    this.#registeredTypes = new Set(projected.map(({ name }) => name));
  }

  /** Keep Sugar first among ordinary definitions so Blueprints remain immediately above it. */
  orderForVue(definitions: UnknownRecord[]): void {
    for (let index = definitions.length - 1; index >= 0; index -= 1) {
      if (isComfyCubePickerType(definitions[index]?.name)) definitions.splice(index, 1);
    }
    definitions.unshift(...this.#registry.definitions().map(toHostDefinition));
  }

  /** Reconcile dynamic additions, updates, and removals before refreshing Comfy consumers. */
  async reconcile(): Promise<void> {
    const refresh = await this.#registry.refresh({ force: true });
    const registerNodeDef = this.#app.registerNodeDef;
    if (typeof registerNodeDef !== 'function') {
      throw new Error('Comfy node-definition registration is unavailable.');
    }
    const nextTypes = new Set(refresh.definitions.map(({ name }) => name));
    for (const staleType of this.#registeredTypes) {
      if (!nextTypes.has(staleType)) this.#liteGraph.unregisterNodeType?.(staleType);
    }
    await Promise.all(
      refresh.definitions.map((definition) =>
        registerNodeDef.call(this.#app, definition.name, toHostDefinition(definition)),
      ),
    );
    this.#registeredTypes = nextTypes;
    if (typeof this.#app.reloadNodeDefs === 'function') {
      await this.#app.reloadNodeDefs();
    }
  }
}

/** Copy the authored definition across the untyped Comfy adapter boundary. */
function toHostDefinition(definition: ComfyCubeNodeDefinition): UnknownRecord {
  const value: unknown = definition;
  if (!isRecord(value)) throw new TypeError('Cube picker definition is invalid.');
  return value;
}
