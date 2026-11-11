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
/** Normalize Comfy's public settings definitions without invoking custom render functions. */
import { isRecord, type UnknownRecord } from '../types/common.js';
import type { SettingsCatalog, SettingsDescriptor } from './SettingsCatalog.js';

/** Retain Comfy as the sole owner of settings values, validation, search and custom pages. */
export class ComfySettingsCatalog implements SettingsCatalog {
  /** Bind the installed host instead of taking a stale copy before plugins register. */
  constructor(private readonly app: unknown) {}

  /** Read one validated descriptor at its mounted surface boundary. */
  find(id: string): SettingsDescriptor | null {
    const settings = this.#settings();
    const lookup: unknown = settings.settingsParamLookup;
    if (!isRecord(lookup)) return null;
    const value: unknown = lookup[id];
    if (!isRecord(value) || value.id !== id || typeof value.name !== 'string') return null;
    return {
      id,
      name: value.name,
      description: typeof value.tooltip === 'string' ? value.tooltip : '',
      category: Array.isArray(value.category)
        ? value.category.filter((part): part is string => typeof part === 'string')
        : id.split('.'),
      kind: typeof value.type === 'function' ? 'custom' : 'standard',
    };
  }

  /** Register additive custom pages through the same host API used by plugins. */
  register(definition: UnknownRecord): void {
    const settings = this.#settings();
    if (typeof settings.addSetting !== 'function')
      throw new Error('Comfy settings registration is unavailable.');
    settings.addSetting.call(settings, definition);
  }

  /** Narrow the legacy public facade where Comfy exposes its registry to extensions. */
  #settings(): UnknownRecord {
    const app = isRecord(this.app) ? this.app : {};
    const ui = isRecord(app.ui) ? app.ui : {};
    return isRecord(ui.settings) ? ui.settings : {};
  }
}
