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
/** Persist Photoshop's interface brightness independently of visual-style choice. */
import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
import { fixed } from '../../settings/workspace/SettingsModel.js';
import { VISUAL_STYLE_SETTING } from './VisualStylePreference.js';
import { isRecord } from '../../types/common.js';
import { isPhotoProPalette, type PhotoProPalette } from './PhotoProThemeTokens.js';
import type { EmbeddedPresentationSource } from '../../core/embedded/EmbeddedPresentation.js';

/** Persist PhotoPro brightness independently of the host color scheme. */
export const PHOTOPRO_PALETTE_SETTING = 'SugarCubes.Appearance.PhotoProPalette';

/** Register the four desktop palettes and expose the accepted selection to CSS. */
export function registerPhotoProPalettePreference(
  app: unknown,
  documentRef: Document,
  report: (error: unknown) => void,
  embedded?: EmbeddedPresentationSource,
): () => void {
  const application = isRecord(app) ? app : {};
  const ui = isRecord(application.ui) ? application.ui : {};
  const settings = isRecord(ui.settings) ? ui.settings : {};
  const read = settings.getSettingValue;
  const register = settings.addSetting;
  const value = typeof read === 'function' ? read.call(settings, PHOTOPRO_PALETTE_SETTING) : null;
  const initial: PhotoProPalette = isPhotoProPalette(value) ? value : 'medium-gray';
  let saved = initial;
  const accept = (palette: PhotoProPalette): void => {
    documentRef.documentElement.dataset.scPhotoproPalette = palette;
  };
  const refresh = (): void => accept(embedded?.read()?.photoProPalette ?? saved);
  if (typeof register === 'function') {
    register.call(settings, {
      id: PHOTOPRO_PALETTE_SETTING,
      name: 'PhotoPro color theme',
      type: 'hidden',
      defaultValue: initial,
      onChange: (candidate: unknown) => {
        if (isPhotoProPalette(candidate)) {
          saved = candidate;
          refresh();
        } else report(new TypeError('PhotoPro color theme must be a supported Photoshop palette.'));
      },
    });
    const store = new ComfySettingStore(app);
    settingsCatalogFor(app).contribute('appearance.style.photopro_palette', {
      controls: [
        {
          kind: 'choice',
          value: store.value(PHOTOPRO_PALETTE_SETTING, (candidate) =>
            isPhotoProPalette(candidate) ? candidate : 'medium-gray',
          ),
          choices: fixed([
            { value: 'black', label: 'Black' },
            { value: 'dark-gray', label: 'Dark Gray' },
            { value: 'medium-gray', label: 'Medium Gray' },
            { value: 'light-gray', label: 'Light Gray' },
          ]),
        },
      ],
      // The palette belongs to the PhotoPro style and shows only while it is chosen.
      visible: {
        read: () => store.get(VISUAL_STYLE_SETTING) === 'photopro',
        subscribe: (listener) => store.subscribe(VISUAL_STYLE_SETTING, listener),
      },
    });
  } else report(new Error('Comfy settings registration is unavailable for PhotoPro palettes.'));
  refresh();
  const stop = embedded?.subscribe(refresh);
  return () => {
    stop?.();
    documentRef.documentElement.removeAttribute('data-sc-photopro-palette');
  };
}
