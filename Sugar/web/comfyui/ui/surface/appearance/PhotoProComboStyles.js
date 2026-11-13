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
/** Paint Comfy's native PhotoPro choices as compact, continuous fields. */
/** Keep Comfy's selection mechanics while replacing its trigger and list paint. */
export function photoProComboCss() {
    return `
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame] { align-self:center; border-radius:3px; }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:hover { --sc-field-surface:var(--sc-surface-hover); }
    html[data-sc-visual-style="photopro"] [data-sc-model-appearance="photopro"] .sc-model-field input:focus-visible { outline:0; }
    html[data-sc-visual-style="photopro"] [data-sc-model-appearance="photopro"] .sc-model-field:has(input:focus-visible) { outline:1px solid var(--sc-focus-ring); outline-offset:1px; }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button {
      height:100%; border:0; border-radius:0; background:transparent; color:inherit; box-shadow:none;
    }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button:hover,
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button:active {
      background:transparent;
    }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > [role="combobox"] { min-width:0; padding:0; font:12px var(--sc-font-family); }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > [role="combobox"] > span { padding:0 4px 0 8px; }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button[aria-hidden="true"] {
      display:grid; place-items:center; flex:0 0 20px; width:20px; padding:0;
    }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button[aria-hidden="true"] > i { display:none; }
    html[data-sc-visual-style="photopro"] [data-sc-style-combo-frame]:not(.sc-model-field) > button[aria-hidden="true"]::after {
      content:''; display:block; box-sizing:border-box; width:6px; height:6px;
      border:0; border-right:1px solid currentColor; border-bottom:1px solid currentColor;
      transform:translateY(-2px) rotate(45deg);
    }
    html[data-sc-visual-style="photopro"] [data-sc-inline-search] { border-radius:3px; }
  `;
}
