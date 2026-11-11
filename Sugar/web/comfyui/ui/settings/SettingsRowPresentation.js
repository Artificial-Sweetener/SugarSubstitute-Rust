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
/** Apply shared Comfy-token row layout to repository settings and pack-management content. */
/** Align settings controls with their explanatory copy inside the available card width. */
export function applySettingsRowShellStyle(element) {
    element.style.cssText =
        'display:flex;flex-direction:column;align-items:flex-start;gap:8px;min-width:0;text-align:left';
}
/** Let explanatory copy inherit the active native or themed text palette. */
export function applySettingsSummaryStyle(element) {
    element.style.cssText =
        'font-size:12px;line-height:1.5;white-space:pre-wrap;color:var(--sc-settings-muted,var(--p-text-muted-color,var(--descrip-text)));max-width:48rem';
}
/** Keep related controls reachable when a narrow settings window wraps. */
export function applySettingsControlsStyle(element) {
    element.style.cssText =
        'display:inline-flex;align-items:center;justify-content:flex-start;gap:8px;flex-wrap:wrap';
}
/** Preserve readable values alongside native action buttons. */
export function applySettingsValueStyle(element) {
    element.style.cssText = 'font-size:14px;line-height:1.5';
}
/** Keep action groups consistent in settings rows and the pack manager. */
export function applySettingsActionGroupStyle(element) {
    element.style.cssText = 'display:flex;align-items:center;gap:8px;flex-wrap:wrap';
}
/** Space pack rows without coupling their layout to the settings page theme. */
export function applySettingsListStyle(element) {
    element.style.cssText = 'display:flex;flex-direction:column;gap:10px';
}
