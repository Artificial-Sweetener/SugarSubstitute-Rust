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
/** Decorate only identified settings rows while retaining all native page and control owners. */
import type { SettingsCatalog } from './SettingsCatalog.js';
import { SettingsSearchPlacement } from './SettingsSearchPlacement.js';
import { applyOwnedTheme } from '../surface/appearance/ThemeService.js';
import { SettingsPagePosition } from './SettingsPagePosition.js';
import { SettingsSwitchControls } from './SettingsSwitchControls.js';

/** Own reversible settings annotations, descriptions and top-search placement for one dialog. */
export class SettingsWorkspaceSurface {
  #placement: SettingsSearchPlacement | null = null;
  #position: SettingsPagePosition | null = null;
  readonly #rows = new Set<HTMLElement>();
  readonly #switches: SettingsSwitchControls;

  /** Bind an actual settings mount rather than discovering arbitrary dialogs or plugin DOM. */
  constructor(
    private readonly root: HTMLElement,
    private readonly catalog: SettingsCatalog,
  ) {
    this.#switches = new SettingsSwitchControls(root);
  }

  /** Apply the selected provider to standard host settings and opt-in SugarCubes content. */
  update(styled: boolean, dark: boolean): void {
    this.root.setAttribute('data-sc-settings', '');
    if (styled) {
      this.root.setAttribute('data-sc-settings-themed', '');
      applyOwnedTheme(this.root, dark);
      this.#placement ??= new SettingsSearchPlacement(this.root);
      this.#position ??= new SettingsPagePosition(this.root);
      if (!this.#placement.reconcile()) this.#position.reconcile();
    } else {
      this.root.removeAttribute('data-sc-settings-themed');
      applyOwnedTheme(this.root, null);
      this.#placement?.dispose();
      this.#placement = null;
      this.#position?.dispose();
      this.#position = null;
    }
    for (const row of this.#rows) {
      if (row.isConnected) continue;
      this.#clearRow(row);
      this.#rows.delete(row);
    }
    for (const row of this.root.querySelectorAll<HTMLElement>('[data-setting-id]')) {
      const id = row.dataset.settingId ?? '';
      const descriptor = this.catalog.find(id);
      if (!descriptor) continue;
      if (descriptor.kind === 'custom' && !id.startsWith('SugarCubes.')) continue;
      this.#rows.add(row);
      row.dataset.scSettingKind = descriptor.kind === 'standard' ? 'standard' : 'owned';
      if (!styled || !descriptor.description) {
        row.querySelector('.sc-setting-description')?.remove();
        continue;
      }
      const label = row.querySelector('.form-label');
      if (!label || label.querySelector('.sc-setting-description')) continue;
      const description = this.root.ownerDocument.createElement('small');
      description.className = 'sc-setting-description sc-settings-owned';
      description.textContent = descriptor.description;
      label.append(description);
    }
    this.#switches.reconcile(styled);
  }

  /** Leave host markup and custom-page lifetimes exactly where Comfy expects them. */
  dispose(): void {
    this.#switches.clear();
    this.#placement?.dispose();
    this.#placement = null;
    this.#position?.dispose();
    this.#position = null;
    for (const row of this.#rows) this.#clearRow(row);
    this.#rows.clear();
    applyOwnedTheme(this.root, null);
    this.root.removeAttribute('data-sc-settings');
    this.root.removeAttribute('data-sc-settings-themed');
  }

  /** Remove only presentation owned by this adapter. */
  #clearRow(row: HTMLElement): void {
    row.removeAttribute('data-sc-setting-kind');
    row.querySelector('.sc-setting-description')?.remove();
  }
}
