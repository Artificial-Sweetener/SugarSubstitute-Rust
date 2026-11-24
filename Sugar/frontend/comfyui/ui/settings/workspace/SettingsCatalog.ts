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
 * Collect the rows features give the settings workspace and lay them out as the Python
 * application's pages. Features own their values and controls; the catalog owns where each row
 * sits and what it says.
 */
import { SETTINGS_LAYOUT, type SettingsRowLayout } from './SettingsLayout.js';
import type { SettingsPage, SettingsRow, SettingsRowContent } from './SettingsModel.js';

const LAYOUT_ROWS: ReadonlyMap<string, SettingsRowLayout> = new Map(
  SETTINGS_LAYOUT.flatMap((page) => page.sections.flatMap((section) => section.rows)).map((row) => [
    row.id,
    row,
  ]),
);

/** One page instance's settings rows, as features contribute them. */
export class SettingsCatalog {
  readonly #content = new Map<string, SettingsRowContent>();
  readonly #listeners = new Set<() => void>();

  /**
   * Give a laid-out row its controls. A row id the layout does not name is a programming error:
   * every row has a place and text before a feature can fill it.
   */
  contribute(id: string, content: SettingsRowContent): () => void {
    if (!LAYOUT_ROWS.has(id)) throw new Error(`Settings row ${id} has no place in the layout.`);
    this.#content.set(id, content);
    this.#changed();
    return () => {
      if (this.#content.get(id) !== content) return;
      this.#content.delete(id);
      this.#changed();
    };
  }

  /** The laid-out row with its contributed controls, or null while no feature has filled it. */
  row(id: string): SettingsRow | null {
    const layout = LAYOUT_ROWS.get(id);
    const content = this.#content.get(id);
    return layout && content ? { ...layout, ...content } : null;
  }

  /** Every page with the rows contributed so far; empty sections and pages are left out. */
  pages(): SettingsPage[] {
    return SETTINGS_LAYOUT.map((page) => ({
      id: page.id,
      title: page.title,
      subtitle: page.subtitle,
      icon: page.icon,
      searchable: page.searchable,
      sections: page.sections
        .map((section) => ({
          id: section.id,
          title: section.title,
          ...(section.subtitle ? { subtitle: section.subtitle } : {}),
          rows: section.rows
            .map((layout) => this.row(layout.id))
            .filter((row): row is SettingsRow => row !== null),
        }))
        .filter((section) => section.rows.length > 0),
    })).filter((page) => page.sections.length > 0);
  }

  /** Follow contributions arriving or leaving. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  #changed(): void {
    for (const listener of [...this.#listeners]) listener();
  }
}

const CATALOGS = new WeakMap<object, SettingsCatalog>();
const NO_APP = {};

/** The settings catalog of one ComfyUI app: every feature of that page contributes to it. */
export function settingsCatalogFor(app: unknown): SettingsCatalog {
  const key = typeof app === 'object' && app !== null ? app : NO_APP;
  let catalog = CATALOGS.get(key);
  if (!catalog) {
    catalog = new SettingsCatalog();
    CATALOGS.set(key, catalog);
  }
  return catalog;
}
