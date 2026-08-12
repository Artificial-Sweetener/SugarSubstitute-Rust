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
/** Own persisted Cube browser favorites, recents, and author-group state. */

import type { BrowserStorage } from './CubeBrowserContracts.js';
import type { CubeBrowserStore } from './CubeBrowserStore.js';

const FAVORITES_KEY = 'sugarcubes.favorites';
const RECENTS_KEY = 'sugarcubes.recent';
const AUTHOR_GROUPS_KEY = 'sugarcubes.author_groups';

interface PreferenceOptions {
  store: CubeBrowserStore;
  storage: BrowserStorage | null;
}

/** Coordinate durable browser preferences independently from catalog retrieval. */
export class CubeBrowserPreferences {
  private readonly store: CubeBrowserStore;
  private readonly storage: BrowserStorage | null;

  constructor({ store, storage }: PreferenceOptions) {
    this.store = store;
    this.storage = storage;
  }

  /** Hydrate all durable browser preferences into the store. */
  initialize(): void {
    this.store.setFavorites(this.readList(FAVORITES_KEY));
    this.store.setRecents(this.readList(RECENTS_KEY));
    this.store.setAuthorGroupsOpen(this.readSet(AUTHOR_GROUPS_KEY));
  }

  /** Toggle the selected Cube favorite and persist the new set. */
  toggleFavorite(cubeKey: string | null): boolean {
    if (!cubeKey) {
      return false;
    }
    const favorites = new Set(this.store.state.favorites);
    if (favorites.has(cubeKey)) {
      favorites.delete(cubeKey);
    } else {
      favorites.add(cubeKey);
    }
    this.store.setFavorites(favorites);
    this.writeList(FAVORITES_KEY, Array.from(favorites));
    return true;
  }

  /** Toggle a durable author-group expansion preference. */
  toggleAuthorGroup(key: string): boolean {
    if (!key) {
      return false;
    }
    const open = new Set(this.store.state.authorGroupsOpen);
    if (open.has(key)) {
      open.delete(key);
    } else {
      open.add(key);
    }
    this.store.setAuthorGroupsOpen(open);
    this.store.setAuthorGroupsTouched(true);
    this.writeSet(AUTHOR_GROUPS_KEY, open);
    return true;
  }

  /** Expand the selected Cube's author group and persist the preference. */
  ensureAuthorGroupOpen(groupKey: string | null): void {
    if (!groupKey) {
      return;
    }
    const open = new Set(this.store.state.authorGroupsOpen);
    if (open.has(groupKey)) {
      return;
    }
    open.add(groupKey);
    this.store.setAuthorGroupsOpen(open);
    this.store.setAuthorGroupsTouched(true);
    this.writeSet(AUTHOR_GROUPS_KEY, open);
  }

  /** Default every discovered author group to open until the user intervenes. */
  ensureAuthorGroupDefaults(keys: readonly string[]): void {
    if (this.store.state.authorGroupsTouched || !keys.length) {
      return;
    }
    this.store.setAuthorGroupsOpen(new Set(keys));
  }

  /** Record one Cube as most recently selected. */
  rememberRecent(cubeKey: string | null): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      return;
    }
    const recents = this.store.state.recents.filter((entry) => entry !== key);
    recents.unshift(key);
    if (recents.length > 12) {
      recents.length = 12;
    }
    this.store.setRecents(recents);
    this.writeList(RECENTS_KEY, recents);
  }

  /** Remove one Cube from preference lists after catalog deletion. */
  forgetCube(cubeKey: string): void {
    const favorites = new Set(this.store.state.favorites);
    if (favorites.delete(cubeKey)) {
      this.store.setFavorites(favorites);
      this.writeList(FAVORITES_KEY, Array.from(favorites));
    }
    const recents = this.store.state.recents.filter((entry) => entry !== cubeKey);
    if (recents.length !== this.store.state.recents.length) {
      this.store.setRecents(recents);
      this.writeList(RECENTS_KEY, recents);
    }
  }

  private readList(key: string): string[] {
    try {
      return this.storage?.readList(key) ?? [];
    } catch (_error) {
      return [];
    }
  }

  private writeList(key: string, values: readonly string[]): void {
    try {
      this.storage?.writeList(key, values);
    } catch (_error) {
      // Preference persistence is best-effort host integration.
    }
  }

  private readSet(key: string): Set<string> {
    try {
      return this.storage?.readSet(key) ?? new Set();
    } catch (_error) {
      return new Set();
    }
  }

  private writeSet(key: string, values: ReadonlySet<string>): void {
    try {
      this.storage?.writeSet(key, values);
    } catch (_error) {
      // Preference persistence is best-effort host integration.
    }
  }
}
