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
/** Apply scoped design-system providers while native controls retain their host value owners. */
import { applyOwnedFluentTheme, fluentTokenDeclarations } from './FluentThemeRuntime.js';
import { spectrumTokenDeclarations } from './SpectrumThemeRuntime.js';
import { fluentAppearanceCss } from './FluentAppearanceStyles.js';
import { spectrumAppearanceCss } from './SpectrumAppearanceStyles.js';
import { readOwnedVisualStyle } from './VisualStyleBoundary.js';
/** Return the complete token contract for one accepted design system and palette. */
export function visualStyleTokens(style, dark) {
    return style === 'spectrum' ? spectrumTokenDeclarations(dark) : fluentTokenDeclarations(dark);
}
/** Apply appearance without changing a surface's DOM identity, focus, or floating transport. */
export function applyOwnedTheme(element, dark, style = readOwnedVisualStyle(element.ownerDocument)) {
    if (style === 'fluent' || dark === null)
        applyOwnedFluentTheme(element, dark);
    else
        applyOwnedFluentTheme(element, null);
    if (dark === null) {
        element.removeAttribute('data-sc-theme');
        element.removeAttribute('data-sc-color-scheme');
        return;
    }
    ensureThemeStyles(element.ownerDocument);
    element.dataset.scTheme = style;
    element.dataset.scColorScheme = dark ? 'dark' : 'light';
}
/** Share native-card geometry while giving Spectrum its official foundations and rounded surfaces. */
export function nativeVisualStyleCss(style, dark) {
    if (style === 'fluent')
        return fluentAppearanceCss(dark);
    return spectrumAppearanceCss(dark);
}
/** Install static provider scopes once so detached controls receive tokens when they connect. */
function ensureThemeStyles(document) {
    if (document.getElementById('sugarcubes-theme-providers'))
        return;
    const stylesheet = document.createElement('style');
    stylesheet.id = 'sugarcubes-theme-providers';
    stylesheet.textContent =
        ['fluent', 'spectrum']
            .flatMap((style) => [true, false].map((dark) => `[data-sc-theme="${style}"][data-sc-color-scheme="${dark ? 'dark' : 'light'}"]{${visualStyleTokens(style, dark)}}`))
            .join('\n') +
            `
    .sc-context-menu[data-sc-theme="spectrum"] {
      padding:4px; border-width:1px; border-style:solid; border-color:var(--sc-border);
      border-radius:var(--sc-control-radius); background:var(--sc-surface-secondary);
      color:var(--sc-text); box-shadow:var(--sc-shadow-medium);
    }
    .sc-substitute-frame[data-sc-theme="spectrum"] {
      background:var(--sc-native-frame-background,var(--sc-surface-secondary));
      --sc-frame-muted:var(--sc-native-frame-foreground,var(--sc-text-secondary));
      --sc-frame-hover:var(--sc-subtle-hover); --sc-frame-pressed:var(--sc-subtle-selected);
    }
    .sc-substitute-frame[data-sc-theme="spectrum"] :is(.sc-workflow-new,.sc-workflow-overflow,.sc-workflow-scroll) {
      color:var(--sc-native-frame-foreground,var(--sc-text));
    }
    [data-sc-theme="spectrum"] [data-sc-workflow-tabs] {
      display:flex; height:30px; width:var(--sc-workflow-tabs-width); min-width:0; overflow:clip;
      --mod-tabs-height:30px; --mod-tabs-font-size:12px;
    }
    [data-sc-theme="spectrum"] [data-sc-workflow-tab] {
      display:inline-flex; align-items:center; box-sizing:border-box; height:30px;
      width:var(--sc-workflow-tab-width,180px); min-width:var(--sc-workflow-tab-width,180px);
      max-width:var(--sc-workflow-tab-width,180px); flex:0 0 var(--sc-workflow-tab-width,180px);
      padding:0 8px 0 20px; border-radius:8px 8px 0 0;
      --mod-tabs-item-height:30px; --mod-tabs-font-size:12px;
    }
    [data-sc-theme="spectrum"] [data-sc-workflow-tab][data-orb-adjacent] { padding-left:32px; }
  `;
    document.head.append(stylesheet);
}
