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
 * Own what the connected host declares for this document.
 *
 * A page outside any host settles at once on the standalone declaration. A page with an
 * identified host transport waits until that host's first answer: its description, or a
 * configure without one, which declares only the panel protocol. The settled declaration is
 * fixed for the document's panel session.
 */
import { DeclaredHost } from './PanelRuntime.js';

/** Let host-dependent owners follow the declaration without sharing mutable panel state. */
export interface HostDescriptionSource {
  /** The settled declaration, or null while an identified host has not answered yet. */
  read(): DeclaredHost | null;
  subscribe(changed: () => void): () => void;
}

/** Publish the declaration once it settles for this frontend lifetime. */
export class HostDescriptionState implements HostDescriptionSource {
  #value: DeclaredHost | null = null;
  readonly #listeners = new Set<() => void>();

  /** Read the settled declaration, or null while it is pending. */
  read(): DeclaredHost | null {
    return this.#value;
  }

  /** Observe the declaration settling. */
  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  /**
   * Settle on a declaration; a later, different one is refused.
   *
   * @returns Whether the declaration is the settled one, so a contradicting host can be reported.
   */
  settle(value: DeclaredHost): boolean {
    if (this.#value) return this.#value.same(value);
    this.#value = value;
    for (const changed of this.#listeners) changed();
    return true;
  }
}
