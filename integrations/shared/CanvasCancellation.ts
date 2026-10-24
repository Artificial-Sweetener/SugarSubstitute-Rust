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
/** Own cancellation for one portable canvas request. */
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
/** Notify native or browser transports without exposing their signal types. */
export class CanvasCancellation implements CanvasCancellationSource {
  #cancelled = false;
  readonly #listeners = new Set<() => void>();
  /** Read cancellation before allocating a resource or committing document changes. */
  cancelled(): boolean {
    return this.#cancelled;
  }
  /** Observe cancellation and immediately notify late subscribers to an already cancelled request. */
  subscribe(listener: () => void): () => void {
    if (this.#cancelled) {
      listener();
      return () => {};
    }
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Cancel once and retire every observer before invoking its callback. */
  cancel(): void {
    if (this.#cancelled) return;
    this.#cancelled = true;
    const listeners = [...this.#listeners];
    this.#listeners.clear();
    for (const listener of listeners) listener();
  }
}
