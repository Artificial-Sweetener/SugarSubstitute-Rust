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
/** Match GIMP's labelled GtkComboBox while retaining the shared choice and model controls. */
import { CONTROL_HEIGHT } from './ControlGeometry.js';
/** Put the caption and right-aligned value inside one flat GTK field and share its arrow treatment. */
export function gimpComboCss() {
    const root = 'html[data-sc-visual-style="gimp"]';
    const layout = `${root} [data-sc-style-combo-layout]`;
    const field = `${layout}:has([data-sc-style-combo-frame]:not([data-sc-model-native]))`;
    const frame = `${root} [data-sc-style-combo-frame]`;
    const arrow = `${frame} > :is(button[aria-hidden="true"],.sc-model-field-arrow)`;
    return `
    ${frame} { display:flex; align-items:center; }
    ${field}, ${frame} { box-sizing:border-box; height:${CONTROL_HEIGHT}; min-height:${CONTROL_HEIGHT}; border:1px solid var(--sc-gimp-edgeBorder); border-radius:5px; background:var(--sc-gimp-widget); color:var(--sc-gimp-foreground); box-shadow:inset 0 1px var(--sc-gimp-combo-highlight); font:12px/18px var(--sc-font-family); }
    ${field} { gap:6px; padding:0 0 0 6px; }
    ${field} > [data-testid="widget-layout-field-label"] { flex:0 1 auto; max-width:45%; padding:0; font-weight:400; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; cursor:default; }
    ${field} > :last-child { flex:1 1 0; max-width:none; min-width:0; }
    ${field} [data-sc-style-combo-frame] { height:100%; min-height:0; border:0; border-radius:0; background:transparent; box-shadow:none; }
    ${field}:hover, ${frame}:hover { background:var(--sc-gimp-selected); box-shadow:none; }
    ${field}:hover [data-sc-style-combo-frame] { background:transparent; }
    ${field}:focus-within, ${frame}:focus-within { outline:1px dotted var(--sc-gimp-foreground); outline-offset:1px; }
    ${field} [data-sc-style-combo-frame]:focus-within { outline:0; }
    ${field} > :last-child > div:focus-within { box-shadow:none; outline:0; }
    ${frame}::after { display:none; }
    ${frame} > :is([role="combobox"],button) { min-width:0; height:100%; margin:0; padding:0; border:0; border-radius:0; background:transparent; color:inherit; box-shadow:none; outline:0; font:inherit; cursor:default; }
    ${frame} > [role="combobox"] { display:flex; flex:1 1 0; }
    ${frame} > [role="combobox"] > span { flex:1; min-width:0; padding:0 4px 0 6px; text-align:start; }
    ${field} [role="combobox"], ${field} [role="combobox"] > span { text-align:right; }
    ${frame}.sc-model-field > input { height:100%; padding:0 4px 0 6px; background:transparent; outline:0; }
    ${root} [data-sc-model-appearance="gimp"] .sc-model-field:has(.sc-model-banner) { color:var(--sc-gimp-foreground); }
    ${frame} .sc-model-banner { display:none; }
    ${arrow} { display:grid; place-items:center; flex:0 0 20px; width:20px; min-width:20px; padding:0; color:inherit; }
    ${arrow} > :is(i,.sc-model-chevron) { display:none; }
    ${arrow}::after { content:''; display:block; box-sizing:border-box; width:6px; height:6px; border:0; border-right:2px solid currentColor; border-bottom:2px solid currentColor; transform:translateY(-2px) rotate(45deg); }
    ${layout}:has([role="combobox"][aria-invalid="true"]), ${frame}:has([role="combobox"][aria-invalid="true"]) { border-color:var(--node-stroke-error,var(--sc-danger-border,#c42b1c)); }
    ${layout}:has([role="combobox"]:disabled), ${layout}:has([role="combobox"][aria-disabled="true"]), ${frame}:has([role="combobox"]:disabled), ${frame}:has([role="combobox"][aria-disabled="true"]) { color:var(--sc-gimp-disabled); background:var(--sc-gimp-widget); }
  `;
}
