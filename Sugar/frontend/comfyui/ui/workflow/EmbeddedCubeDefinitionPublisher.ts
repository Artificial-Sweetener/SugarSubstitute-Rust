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
/** Publish validated embedded definitions before Comfy constructs native nodes. */

import { EmbeddedCubeDefinitionResolver } from './EmbeddedCubeDefinitionResolver.js';
import type { ResolvedEmbeddedCubeDefinition } from './EmbeddedCubeDefinitionResolver.js';
import type { UnknownRecord } from '../types/common.js';

export interface EmbeddedDefinitionStore {
  clearEmbedded(): number;
  publishEmbedded(
    request: ResolvedEmbeddedCubeDefinition,
    payload: UnknownRecord,
    contentFingerprint: string,
  ): unknown;
}

/** Coordinate embedded workflow recognition with the selected-definition cache. */
export class EmbeddedCubeDefinitionPublisher {
  readonly #store: EmbeddedDefinitionStore;
  readonly #resolver: EmbeddedCubeDefinitionResolver;

  /** Bind the cache owner and pure workflow resolver. */
  constructor(store: EmbeddedDefinitionStore, resolver = new EmbeddedCubeDefinitionResolver()) {
    this.#store = store;
    this.#resolver = resolver;
  }

  /** Publish every workflow-owned definition and return the published count. */
  publish(workflow: unknown): number {
    const definitions = this.#resolver.resolve(workflow);
    this.#store.clearEmbedded();
    for (const definition of definitions) {
      this.#store.publishEmbedded(definition, definition.payload, definition.contentFingerprint);
    }
    return definitions.length;
  }
}
