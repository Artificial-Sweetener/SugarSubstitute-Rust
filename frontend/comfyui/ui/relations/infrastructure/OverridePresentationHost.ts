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
/** Select one override presentation while preserving independent frame and Plugin Mode preferences. */
import type { GlobalOverrideToolbar } from '../presentation/GlobalOverrideToolbar.js';
import { registerPluginModePreference } from '../../surface/frame/PluginModePreference.js';

/** Keep mode precedence at one owner instead of competing frame callbacks. */
export class OverridePresentationHost {
  #inline = false;
  #plugin = false;
  #release: (() => void) | null = null;

  /** Bind the existing controls without creating another relation or value owner. */
  constructor(
    private readonly toolbar: Pick<GlobalOverrideToolbar, 'setPresentation' | 'reconcile'>,
  ) {}

  /** Restore Plugin Mode once Comfy settings are ready. */
  setup(app: unknown, document: Document, report: (error: unknown) => void): void {
    if (this.#release) return;
    this.#release = registerPluginModePreference(
      app,
      document,
      (enabled) => {
        this.#plugin = enabled;
        this.#present();
      },
      report,
    );
  }

  /** Retain frame intent while Plugin Mode owns the override card. */
  setInline(inline: boolean): void {
    this.#inline = inline;
    this.#present();
  }

  /** Release document presentation without changing the saved host preference. */
  dispose(): void {
    this.#release?.();
    this.#release = null;
  }

  /** Apply mode precedence through the existing control presentation boundary. */
  #present(): void {
    this.toolbar.setPresentation(this.#plugin ? 'plugin' : this.#inline ? 'substitute' : 'comfy');
    this.toolbar.reconcile();
  }
}
