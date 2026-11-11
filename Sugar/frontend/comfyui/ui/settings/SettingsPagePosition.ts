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
/** Retain each native settings page's reading position without caching or rebuilding page controls. */
import { readSettingsCategory } from './ComfySettingsNavigation.js';

/** Keep native page changes from inheriting another page's scroll offset. */
export class SettingsPagePosition {
  readonly #positions = new Map<string, number>();
  #key = '';
  #scroll: HTMLElement | null = null;
  #pending = false;

  /** Observe intentional navigation before Comfy replaces or clamps the current page contents. */
  constructor(private readonly root: HTMLElement) {
    root.addEventListener('click', this.#intent, true);
    root.addEventListener('input', this.#intent, true);
  }

  /** Restore only when the native category or its global search view actually changes. */
  reconcile(): void {
    const input = this.root.querySelector<HTMLInputElement>('input[placeholder]');
    const key = input?.value.trim() ? 'search' : readSettingsCategory(this.root)?.id;
    const scroll = this.root.querySelector<HTMLElement>('main > div');
    if (!key || !scroll) return;
    if (scroll !== this.#scroll) {
      this.#scroll?.removeEventListener('scroll', this.#remember);
      this.#scroll = scroll;
      scroll.addEventListener('scroll', this.#remember);
    }
    if (key !== this.#key) {
      this.#key = key;
      scroll.scrollTop = this.#positions.get(key) ?? 0;
    }
    this.#pending = false;
  }

  /** Release scoped listeners while leaving the user's current reading position unchanged. */
  dispose(): void {
    this.root.removeEventListener('click', this.#intent, true);
    this.root.removeEventListener('input', this.#intent, true);
    this.#scroll?.removeEventListener('scroll', this.#remember);
  }

  /** Record primary page scrolling without listening to nested plugin scroll surfaces. */
  readonly #remember = (): void => {
    if (this.#key && this.#scroll && !this.#pending)
      this.#positions.set(this.#key, this.#scroll.scrollTop);
  };

  /** Freeze the outgoing offset before native page rendering can shorten the scroll extent. */
  readonly #intent = (event: Event): void => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target?.closest('[data-nav-id], .sc-settings-search-bar')) return;
    this.#remember();
    this.#pending = true;
  };
}
