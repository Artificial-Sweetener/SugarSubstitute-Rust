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
/** Keep borrowed native controls aligned with their actual owned surface palette. */
export function ensureOwnedControlAppearanceStyles(document: Document): void {
  if (document.getElementById('sugarcubes-owned-control-appearance')) return;
  const style = document.createElement('style');
  style.id = 'sugarcubes-owned-control-appearance';
  style.textContent = `
    :is(.sugarcubes-native-node-card, .sugarcubes-native-widget, .sc-field-relation, [data-sc-style-node], [data-sc-style-surface])
      :is(input, textarea, [role="combobox"]):not(.sc-model-field input) {
      color:var(--colorNeutralForeground1,var(--fg-color,#eee));
    }
  `;
  document.head.append(style);
}
