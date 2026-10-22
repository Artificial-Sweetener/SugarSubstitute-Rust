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
/** Own the opt-in Plugin Mode setting independently of workflow data and visual style. */
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../../types/common.js';
/** Persist the opt-in mode with Comfy's appearance preferences. */
export const PLUGIN_MODE_SETTING = 'SugarCubes.Appearance.PluginMode';
/** Restore the host preference and publish only validated presentation changes. */
export function registerPluginModePreference(app, document, changed, report) {
    const ui = isRecord(app) && isRecord(app.ui) ? app.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    const accept = (value) => {
        if (typeof value !== 'boolean') {
            report(new TypeError('Plugin Mode must be enabled or disabled.'));
            return;
        }
        document.documentElement.toggleAttribute('data-sc-plugin-mode', value);
        changed(value);
    };
    if (typeof settings.addSetting === 'function') {
        settings.addSetting.call(settings, {
            id: PLUGIN_MODE_SETTING,
            name: 'Plugin Mode',
            ...sugarCubesSettingsSection('appearance', 'PluginMode'),
            type: 'boolean',
            defaultValue: false,
            tooltip: 'Show global overrides as an always-expanded node card above the cubes.',
            onChange: accept,
        });
    }
    else
        report(new Error('Comfy settings registration is unavailable for Plugin Mode.'));
    accept(typeof settings.getSettingValue === 'function' &&
        settings.getSettingValue.call(settings, PLUGIN_MODE_SETTING) === true);
    return () => document.documentElement.removeAttribute('data-sc-plugin-mode');
}
