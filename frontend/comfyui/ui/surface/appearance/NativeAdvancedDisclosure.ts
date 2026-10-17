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
/** Compose Fluent advanced disclosure for native graph and stable projected mounts. */
import type { NativeCardBoundary } from './NativeCardAdapter.js';
import { NativeGraphAdvancedDisclosure } from './NativeGraphAdvancedDisclosure.js';
import { NativeProjectedAdvancedDisclosure } from './NativeProjectedAdvancedDisclosure.js';
import { readProjectedAdvancedInputMount } from '../NativeProjectedAdvancedInputs.js';

/** Route presentation to the mounted state owner rather than competing click handlers. */
export class NativeAdvancedDisclosure {
  readonly #graph = new NativeGraphAdvancedDisclosure();
  readonly #projected = new NativeProjectedAdvancedDisclosure();

  /** Reconcile each recognized card through its renderer's disclosure lifetime. */
  reconcile(card: NativeCardBoundary): void {
    const projection = readProjectedAdvancedInputMount(card.root);
    if (projection) this.#projected.reconcile(projection, card);
    else this.#graph.reconcile(card);
  }

  /** Release both presentations while retaining the latest native advanced state. */
  clear(): void {
    this.#graph.clear();
    this.#projected.clear();
  }
}
