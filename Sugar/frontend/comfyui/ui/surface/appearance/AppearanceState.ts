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

import type { VisualStyle } from './VisualStyle.js';

/** Describe the appearance accepted by the native presentation owner. */
export interface AppearanceSnapshot {
  readonly style: VisualStyle;
  readonly dark: boolean;
}

/** Subscribe owned surfaces to the authoritative appearance lifecycle. */
export interface AppearanceSource {
  readonly appearance: AppearanceSnapshot;
  subscribeAppearance(listener: () => void): () => void;
}

/** Publish coherent presentation facts only when their accepted values change. */
export class AppearanceState implements AppearanceSource {
  readonly #listeners = new Set<() => void>();
  #appearance: AppearanceSnapshot = Object.freeze({ style: 'comfy', dark: true });

  /** Expose an immutable accepted snapshot to isolated owned surfaces. */
  get appearance(): AppearanceSnapshot {
    return this.#appearance;
  }

  /** Retain one observer until its owning surface releases the subscription. */
  subscribeAppearance(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Publish only facts supplied by the existing appearance owner. */
  update(style: VisualStyle, dark: boolean): void {
    if (style === this.#appearance.style && dark === this.#appearance.dark) return;
    this.#appearance = Object.freeze({ style, dark });
    for (const listener of this.#listeners) listener();
  }

  /** Release surface subscriptions with the host appearance lifetime. */
  dispose(): void {
    this.#listeners.clear();
  }
}
