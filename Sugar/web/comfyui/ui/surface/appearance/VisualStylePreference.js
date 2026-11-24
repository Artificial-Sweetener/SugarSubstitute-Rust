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
/** Adapt Comfy's persisted settings to one validated visual-style preference. */
import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
import { fixed } from '../../settings/workspace/SettingsModel.js';
import { isRecord } from '../../types/common.js';
import { isVisualStyle, resolveVisualStyle } from './VisualStyle.js';
/** Persist visual style independently of workflow documents and renderer selection. */
export const VISUAL_STYLE_SETTING = 'SugarCubes.Appearance.VisualStyle';
const LEGACY_FLUENT_SETTING = 'SugarCubes.Appearance.Fluent';
/** Read settings at the host boundary without allowing dynamic values into presentation state. */
export function readVisualStylePreference(app) {
    const settings = settingsBoundary(app);
    if (typeof settings.getSettingValue !== 'function')
        return 'comfy';
    return resolveVisualStyle(settings.getSettingValue.call(settings, VISUAL_STYLE_SETTING), settings.getSettingValue.call(settings, LEGACY_FLUENT_SETTING));
}
/** Register the visual-style combobox while retaining the public legacy preference as a default. */
export function registerVisualStylePreference(app, accept, report, embedded) {
    const settings = settingsBoundary(app);
    if (typeof settings.addSetting !== 'function') {
        report(new Error('Comfy settings registration is unavailable for visual style.'));
        return () => undefined;
    }
    const initial = readVisualStylePreference(app);
    let saved = initial;
    let applied = null;
    const refresh = () => {
        const style = embedded?.read()?.visualStyle ?? saved;
        if (style === applied)
            return;
        applied = style;
        accept(style);
    };
    settings.addSetting.call(settings, {
        id: VISUAL_STYLE_SETTING,
        name: 'Visual style',
        type: 'hidden',
        defaultValue: initial,
        onChange: (value) => {
            if (isVisualStyle(value)) {
                saved = value;
                refresh();
            }
            else
                report(new TypeError('The visual style must be Comfy, Fluent, Spectrum 2, PhotoPro, or GIMP.'));
        },
    });
    refresh();
    settingsCatalogFor(app).contribute('appearance.style.visual', {
        controls: [
            {
                kind: 'choice',
                value: new ComfySettingStore(app).value(VISUAL_STYLE_SETTING, (value) => isVisualStyle(value) ? value : 'comfy'),
                choices: fixed([
                    { value: 'comfy', label: 'Comfy' },
                    { value: 'fluent', label: 'Fluent' },
                    { value: 'spectrum', label: 'Spectrum 2' },
                    { value: 'photopro', label: 'PhotoPro' },
                    { value: 'gimp', label: 'GIMP' },
                ]),
            },
        ],
        // A host that decides the style itself leaves nothing to choose here.
        ...(embedded
            ? {
                enabled: {
                    read: () => embedded.read()?.visualStyle === undefined,
                    subscribe: (listener) => embedded.subscribe(listener),
                },
            }
            : {}),
    });
    return embedded?.subscribe(refresh) ?? (() => undefined);
}
/** Narrow Comfy's dynamic settings object before invoking host methods. */
function settingsBoundary(app) {
    const application = isRecord(app) ? app : {};
    const ui = isRecord(application.ui) ? application.ui : {};
    return isRecord(ui.settings) ? ui.settings : {};
}
