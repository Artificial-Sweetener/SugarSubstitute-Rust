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
/** Own the style-independent compact masonry preference and its layout signal. */
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../../types/common.js';

/** Persist density outside Cube documents. */
export const COMPACT_MASONRY_SETTING = 'SugarCubes.Appearance.CompactMasonry';
/** Notify mounted masonry surfaces to recompute their geometry. */
export const COMPACT_MASONRY_CHANGE = 'sugarcubes-compact-masonry-change';

/** Restore one validated setting without storing density in workflow documents. */
export function registerCompactMasonryPreference(
  app: unknown,
  documentRef: Document,
  report: (error: unknown) => void,
): () => void {
  const application = isRecord(app) ? app : {};
  const ui = isRecord(application.ui) ? application.ui : {};
  const settings = isRecord(ui.settings) ? ui.settings : {};
  const read = settings.getSettingValue;
  const register = settings.addSetting;
  const initial =
    typeof read === 'function' && read.call(settings, COMPACT_MASONRY_SETTING) === true;
  const accept = (value: boolean): void => {
    documentRef.documentElement.toggleAttribute('data-sc-compact-masonry', value);
    documentRef.dispatchEvent(new Event(COMPACT_MASONRY_CHANGE));
  };
  if (typeof register === 'function') {
    register.call(settings, {
      id: COMPACT_MASONRY_SETTING,
      name: 'Compact masonry',
      ...sugarCubesSettingsSection('appearance', 'CompactMasonry'),
      type: 'boolean',
      defaultValue: initial,
      tooltip: 'Pack node cards edge to edge with square corners in every visual style.',
      onChange: (value: unknown) => {
        if (typeof value === 'boolean') accept(value);
        else report(new TypeError('Compact masonry must be enabled or disabled.'));
      },
    });
  } else report(new Error('Comfy settings registration is unavailable for compact masonry.'));
  accept(initial);
  return () => documentRef.documentElement.removeAttribute('data-sc-compact-masonry');
}
