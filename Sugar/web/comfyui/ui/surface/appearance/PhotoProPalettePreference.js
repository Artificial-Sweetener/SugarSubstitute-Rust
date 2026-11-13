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
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../../types/common.js';
import { isPhotoProPalette } from './PhotoProThemeTokens.js';
/** Persist PhotoPro brightness independently of the host color scheme. */
export const PHOTOPRO_PALETTE_SETTING = 'SugarCubes.Appearance.PhotoProPalette';
/** Register the four desktop palettes and expose the accepted selection to CSS. */
export function registerPhotoProPalettePreference(app, documentRef, report, embedded) {
    const application = isRecord(app) ? app : {};
    const ui = isRecord(application.ui) ? application.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    const read = settings.getSettingValue;
    const register = settings.addSetting;
    const value = typeof read === 'function' ? read.call(settings, PHOTOPRO_PALETTE_SETTING) : null;
    const initial = isPhotoProPalette(value) ? value : 'medium-gray';
    let saved = initial;
    const accept = (palette) => {
        documentRef.documentElement.dataset.scPhotoproPalette = palette;
    };
    const refresh = () => accept(embedded?.read()?.photoProPalette ?? saved);
    if (typeof register === 'function') {
        register.call(settings, {
            id: PHOTOPRO_PALETTE_SETTING,
            name: 'PhotoPro color theme',
            ...sugarCubesSettingsSection('appearance', 'PhotoProPalette'),
            type: 'combo',
            defaultValue: initial,
            options: [
                { value: 'black', text: 'Black' },
                { value: 'dark-gray', text: 'Dark Gray' },
                { value: 'medium-gray', text: 'Medium Gray' },
                { value: 'light-gray', text: 'Light Gray' },
            ],
            tooltip: 'Choose one of Photoshop desktop’s four interface brightness levels.',
            onChange: (candidate) => {
                if (isPhotoProPalette(candidate)) {
                    saved = candidate;
                    refresh();
                }
                else
                    report(new TypeError('PhotoPro color theme must be a supported Photoshop palette.'));
            },
        });
    }
    else
        report(new Error('Comfy settings registration is unavailable for PhotoPro palettes.'));
    refresh();
    const stop = embedded?.subscribe(refresh);
    return () => {
        stop?.();
        documentRef.documentElement.removeAttribute('data-sc-photopro-palette');
    };
}
