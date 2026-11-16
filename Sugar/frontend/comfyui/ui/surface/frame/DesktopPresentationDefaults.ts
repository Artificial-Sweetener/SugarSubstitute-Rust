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
/** Give a desktop host's install the presentation the desktop application is designed around. */
import { VUE_NODES_SETTING_ID } from '../../core/ComfyRendererMode.js';
import { isRecord } from '../../types/common.js';
import { VISUAL_STYLE_SETTING } from '../appearance/VisualStylePreference.js';

/** The hidden setting recording which defaults revision this install has received. */
export const DESKTOP_DEFAULTS_SETTING = 'SugarCubes.Desktop.PresentationDefaults';

/** Raised when the defaults change, so installs that received an earlier set receive the new one. */
const REVISION = 1;

/**
 * Select the Fluent visual style and Comfy's Vue node renderer the first time a desktop host
 * presents this install.
 *
 * The desktop frame itself is held on by the host for as long as it presents the page. These two
 * are ordinary saved settings instead: they are written once, recorded in a hidden setting, and
 * every later choice the person makes in the settings dialog is kept.
 */
export class DesktopPresentationDefaults {
  #applied = false;

  /** Borrow the host application's settings without touching them until a desktop host asks. */
  constructor(
    private readonly app: unknown,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Apply the defaults unless this install has already received them. */
  apply(): void {
    if (this.#applied) return;
    this.#applied = true;
    const host = isRecord(this.app) ? this.app : {};
    const ui = isRecord(host.ui) ? host.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : null;
    const add = settings?.addSetting,
      get = settings?.getSettingValue,
      set = settings?.setSettingValue;
    if (
      !settings ||
      typeof add !== 'function' ||
      typeof get !== 'function' ||
      typeof set !== 'function'
    ) {
      this.report(new Error('Comfy settings are unavailable for desktop presentation defaults.'));
      return;
    }
    (add as (value: unknown) => unknown).call(settings, {
      id: DESKTOP_DEFAULTS_SETTING,
      name: 'Desktop presentation defaults already applied',
      type: 'hidden',
      defaultValue: 0,
    });
    const received: unknown = (get as (id: string) => unknown).call(
      settings,
      DESKTOP_DEFAULTS_SETTING,
    );
    if (typeof received === 'number' && received >= REVISION) return;
    const write = async (id: string, value: unknown): Promise<void> => {
      await (set as (id: string, value: unknown) => unknown).call(settings, id, value);
    };
    void Promise.all([write(VISUAL_STYLE_SETTING, 'fluent'), write(VUE_NODES_SETTING_ID, true)])
      .then(() => write(DESKTOP_DEFAULTS_SETTING, REVISION))
      .catch((error: unknown) => {
        // Nothing was recorded, so the next launch applies the defaults again.
        this.report(error);
      });
  }
}
