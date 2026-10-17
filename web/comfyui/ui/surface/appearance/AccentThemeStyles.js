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
/** Bridge one accepted accent palette into every SugarCubes-owned rendering surface. */
import { accentForeground, accentIndicatorColor, mixAccentColor } from './AccentPalette.js';
import { fluentAccentColorTokens } from './FluentThemeRuntime.js';
const scopes = ':is(#graph-canvas-container, [data-sc-style-theme-root], [data-sc-style-surface], [data-sc-style-popup], [data-sc-floating-surface], [data-sc-modal-appearance], [data-sc-model-appearance], [id^="SugarCubes."], .sugarcubes-cube-face, .sugarcubes-workflow-masonry, .sc-cube-stack-host, .sc-overrides-toolbar, .sugarcubes-settings-row)';
/** Bridge the accepted accent into scoped provider tokens without overwriting unrelated host themes. */
export function accentThemeCss(color, dark) {
    const tokens = fluentAccentColorTokens(color, dark);
    const declarations = Object.entries(tokens)
        .map(([name, value]) => `--${name}:${value} !important;`)
        .join('');
    const hover = mixAccentColor(color, '#000000', 0.08);
    const pressed = mixAccentColor(color, '#000000', 0.16);
    return `:root {
    --sc-accent-color:${color};
    --sc-accent-indicator:${accentIndicatorColor(color, dark)};
    --sc-accent-foreground:${accentForeground(color)};
    --sc-accent-hover:${hover};
    --sc-accent-pressed:${pressed};
  }
  ${scopes}, ${scopes} [data-fluent-theme], [data-sc-theme] {
    ${declarations}
    --sc-brand-fill:var(--sc-accent-color) !important;
    --sc-brand-fill-hover:var(--sc-accent-hover) !important;
    --sc-brand-fill-pressed:var(--sc-accent-pressed) !important;
    --sc-brand-text:var(--sc-accent-indicator) !important;
    --sc-brand-border:var(--sc-accent-indicator) !important;
    --sc-brand-indicator:var(--sc-accent-indicator) !important;
    --sc-brand-indicator-hover:var(--sc-accent-hover) !important;
    --sc-focus-ring:var(--sc-accent-indicator) !important;
    --spectrum-focus-indicator-color:var(--sc-accent-indicator) !important;
    --sc-text-on-accent:var(--sc-accent-foreground) !important;
    --spectrum-accent-color-700:var(--sc-accent-indicator) !important;
    --spectrum-accent-color-800:var(--sc-accent-indicator) !important;
    --spectrum-accent-color-900:var(--sc-accent-color) !important;
    --spectrum-accent-color-1000:var(--sc-accent-hover) !important;
    --spectrum-accent-color-1100:var(--sc-accent-pressed) !important;
    --primary-color:var(--sc-accent-color) !important;
    --p-primary-color:var(--sc-accent-color) !important;
    --p-primary-hover-color:var(--sc-accent-hover) !important;
    --p-primary-active-color:var(--sc-accent-pressed) !important;
    --p-primary-contrast-color:var(--sc-accent-foreground) !important;
    --primary-background:var(--sc-accent-color) !important;
    --primary-background-hover:var(--sc-accent-hover) !important;
    --primary-foreground:var(--sc-accent-foreground) !important;
    --p-toggleswitch-checked-background:var(--sc-accent-indicator) !important;
    --p-toggleswitch-checked-hover-background:var(--sc-accent-hover) !important;
    --p-checkbox-checked-background:var(--sc-accent-indicator) !important;
    --p-checkbox-checked-border-color:var(--sc-accent-indicator) !important;
    --p-checkbox-checked-hover-background:var(--sc-accent-hover) !important;
    --p-checkbox-checked-hover-border-color:var(--sc-accent-hover) !important;
    --p-checkbox-icon-checked-color:${accentForeground(accentIndicatorColor(color, dark))} !important;
    --p-radiobutton-checked-background:var(--sc-accent-indicator) !important;
    --p-radiobutton-checked-border-color:var(--sc-accent-indicator) !important;
    --p-slider-range-background:var(--sc-accent-indicator) !important;
    --p-button-primary-color:var(--sc-accent-foreground) !important;
    --p-button-primary-background:var(--sc-accent-color) !important;
    --p-button-primary-hover-background:var(--sc-accent-hover) !important;
    --p-button-primary-hover-color:var(--sc-accent-foreground) !important;
    --p-inputtext-focus-border-color:var(--sc-accent-indicator) !important;
    --p-select-focus-border-color:var(--sc-accent-indicator) !important;
    --component-node-widget-background-highlighted:var(--sc-accent-indicator) !important;
    --component-node-widget-advanced:var(--sc-accent-indicator) !important;
    --node-component-outline:var(--sc-accent-indicator) !important;
  }
  ${scopes} [data-variant="primary"] { color:var(--sc-accent-foreground); }
  .sugarcubes-settings-row input[type="checkbox"] { accent-color:var(--sc-accent-color); }
  .sc-accent-setting :is(input,button) {
    height:32px; border:1px solid var(--p-content-border-color,var(--border-color,#555));
    border-radius:4px; background:var(--p-inputtext-background,var(--comfy-input-bg,#303030));
    color:var(--p-inputtext-color,var(--fg-color,#eee)); font:inherit;
  }
  .sc-accent-setting input[type="color"] { width:40px; padding:3px; cursor:pointer; }
  .sc-accent-setting input[type="text"] { width:110px; padding:0 8px; }
  .sc-accent-setting button { padding:0 12px; cursor:pointer; }
  .sc-accent-setting :is(input,button):focus-visible {
    outline:2px solid var(--sc-accent-indicator); outline-offset:2px;
  }`;
}
