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
/** Share foregrounds, numeric pair geometry and seed accessories across node renderers. */
import { nativeSeedCss } from './NativeSeedStyles.js';
import { CONTROL_HEIGHT, SPINNER_WIDTH } from './ControlGeometry.js';
/** Keep borrowed native controls aligned with their actual owned surface palette. */
export function ensureOwnedControlAppearanceStyles(document) {
    const existing = document.getElementById('sugarcubes-owned-control-appearance');
    if (existing) {
        document.head.append(existing);
        return;
    }
    const style = document.createElement('style');
    style.id = 'sugarcubes-owned-control-appearance';
    style.textContent = `
    :is(.sugarcubes-native-node-card, .sugarcubes-native-widget, .sc-field-relation, [data-sc-style-node], [data-sc-style-surface])
      :is(input, textarea, [role="combobox"]):not(.sc-model-field input) {
      color:var(--sc-text,var(--fg-color,#eee));
    }
    ${nativeSeedCss()}
    .sugarcubes-cube-face__node-card [data-sc-style-layout]:has([data-sc-field-action]) { display:flex; }
    .sugarcubes-cube-face__node-card [data-sc-style-layout]:has([data-sc-field-action]) > [data-testid="widget-layout-field-label"] { display:none; }
    .sugarcubes-cube-face__node-card [data-sc-style-row] [data-sc-style-layout]:has([data-sc-field-action]) > :last-child { flex:1 1 auto; width:100%; max-width:none; grid-column:1 / -1; }
    [data-sc-field-action] { display:flex; align-items:center; gap:4px; min-width:0; }
    [data-sc-field-action] > input { flex:1 1 0; width:0; min-width:0 !important; height:${CONTROL_HEIGHT}; box-sizing:border-box; padding-block:0; text-overflow:ellipsis; }
    [data-sc-field-action] > [data-sc-button] { flex:none; margin:0; padding:0 6px; height:${CONTROL_HEIGHT}; min-height:0; font:inherit; }
    [data-sc-number-pair] { display:flex; align-items:center; justify-content:flex-end; min-width:0; gap:6px; }
    [data-sc-number-pair] > :first-child { width:${SPINNER_WIDTH}; min-width:${SPINNER_WIDTH}; flex:0 0 ${SPINNER_WIDTH}; }
    [data-sc-number-slider] { width:120px; min-width:40px; max-width:120px; flex:1 1 120px; }
    .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > [data-sc-number-pair] {
      flex:0 0 calc(${SPINNER_WIDTH} + 126px); max-width:calc(${SPINNER_WIDTH} + 126px);
    }
  `;
    document.head.append(style);
}
