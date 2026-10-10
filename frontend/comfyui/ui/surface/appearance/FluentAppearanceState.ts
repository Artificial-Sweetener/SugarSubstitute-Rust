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
/** Share accepted appearance facts without introducing another settings or palette owner. */

/** Describe the appearance accepted by the native presentation owner. */
export interface FluentAppearanceSnapshot {
  readonly fluent: boolean;
  readonly dark: boolean;
}

/** Subscribe owned surfaces to the authoritative appearance lifecycle. */
export interface FluentAppearanceSource {
  readonly appearance: FluentAppearanceSnapshot;
  subscribeAppearance(listener: () => void): () => void;
}

/** Publish coherent presentation facts only when their accepted values change. */
export class FluentAppearanceState implements FluentAppearanceSource {
  readonly #listeners = new Set<() => void>();
  #appearance: FluentAppearanceSnapshot = Object.freeze({ fluent: false, dark: true });

  /** Expose an immutable accepted snapshot to isolated owned surfaces. */
  get appearance(): FluentAppearanceSnapshot {
    return this.#appearance;
  }

  /** Retain one observer until its owning surface releases the subscription. */
  subscribeAppearance(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Publish only facts supplied by the existing appearance owner. */
  update(fluent: boolean, dark: boolean): void {
    if (fluent === this.#appearance.fluent && dark === this.#appearance.dark) return;
    this.#appearance = Object.freeze({ fluent, dark });
    for (const listener of this.#listeners) listener();
  }

  /** Release surface subscriptions with the host appearance lifetime. */
  dispose(): void {
    this.#listeners.clear();
  }
}
