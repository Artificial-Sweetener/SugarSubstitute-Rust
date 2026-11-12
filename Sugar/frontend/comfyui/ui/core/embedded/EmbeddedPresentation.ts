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
/** Own one document-local host presentation without writing persisted Comfy settings. */
import type { PanelPresentation } from './PanelRuntime.js';
import { freezePanelPresentation, samePanelPresentation } from './PanelRuntime.js';

/** Let independent preference owners follow the same accepted host session. */
export interface EmbeddedPresentationSource {
  read(): PanelPresentation | null;
  subscribe(changed: () => void): () => void;
}

/** Publish only negotiated presentation changes for this frontend lifetime. */
export class EmbeddedPresentation implements EmbeddedPresentationSource {
  #value: PanelPresentation | null = null;
  readonly #listeners = new Set<() => void>();

  /** Read the immutable accepted host configuration. */
  read(): PanelPresentation | null {
    return this.#value;
  }

  /** Observe preference precedence without sharing mutable host state. */
  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  /** Apply or release a host session while preserving each owner's saved preference. */
  set(value: PanelPresentation | null): void {
    if (samePanelPresentation(value, this.#value)) return;
    this.#value = value ? freezePanelPresentation(value) : null;
    for (const changed of this.#listeners) changed();
  }
}
