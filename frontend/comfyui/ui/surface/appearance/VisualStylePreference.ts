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
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';
import { isVisualStyle, resolveVisualStyle, type VisualStyle } from './VisualStyle.js';

/** Persist visual style independently of workflow documents and renderer selection. */
export const VISUAL_STYLE_SETTING = 'SugarCubes.Appearance.VisualStyle';
const LEGACY_FLUENT_SETTING = 'SugarCubes.Appearance.Fluent';

/** Read settings at the host boundary without allowing dynamic values into presentation state. */
export function readVisualStylePreference(app: unknown): VisualStyle {
  const settings = settingsBoundary(app);
  if (typeof settings.getSettingValue !== 'function') return 'comfy';
  return resolveVisualStyle(
    settings.getSettingValue.call(settings, VISUAL_STYLE_SETTING),
    settings.getSettingValue.call(settings, LEGACY_FLUENT_SETTING),
  );
}

/** Register the visual-style combobox while retaining the public legacy preference as a default. */
export function registerVisualStylePreference(
  app: unknown,
  accept: (style: VisualStyle) => void,
  report: (error: unknown) => void,
): void {
  const settings = settingsBoundary(app);
  if (typeof settings.addSetting !== 'function') {
    report(new Error('Comfy settings registration is unavailable for visual style.'));
    return;
  }
  const initial = readVisualStylePreference(app);
  settings.addSetting.call(settings, {
    id: VISUAL_STYLE_SETTING,
    name: 'Visual style',
    ...sugarCubesSettingsSection('appearance', 'VisualStyle'),
    type: 'combo',
    defaultValue: initial,
    options: [
      { value: 'comfy', text: 'Comfy' },
      { value: 'fluent', text: 'Fluent' },
      { value: 'spectrum', text: 'Spectrum 2' },
    ],
    tooltip:
      'Choose Comfy, Fluent, or Spectrum 2 for node cards, masonry, and SugarCubes surfaces.',
    onChange: (value: unknown) => {
      if (isVisualStyle(value)) accept(value);
      else report(new TypeError('The visual style must be Comfy, Fluent, or Spectrum 2.'));
    },
  });
  accept(initial);
}

/** Narrow Comfy's dynamic settings object before invoking host methods. */
function settingsBoundary(app: unknown): UnknownRecord {
  const application = isRecord(app) ? app : {};
  const ui = isRecord(application.ui) ? application.ui : {};
  return isRecord(ui.settings) ? ui.settings : {};
}
