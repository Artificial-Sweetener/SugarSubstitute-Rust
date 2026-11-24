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
/**
 * One settings look per visual style. The workspace's structure (Python's pages, sections,
 * cards and behaviour) is the same in every style; its colours and geometry come from the
 * active style. Fluent is the Python application's WinUI settings, measured; PhotoPro, GIMP and
 * Spectrum build on their own theme tokens, which the workspace root receives from the active
 * style; Comfy follows ComfyUI's palette.
 */
import type { VisualStyle } from '../../surface/appearance/VisualStyle.js';

/** The settings tokens, as CSS custom properties `--sc-sw-<name>`. */
export const SETTINGS_TOKENS = [
  'page',
  'text',
  'card',
  'border',
  'radius',
  'hover',
  'press',
  'nav-selected',
  'accent',
  'control',
  'control-hover',
  'control-press',
  'control-border',
  'control-edge',
  'control-radius',
  'field-edge',
  'field-focus',
  'disabled',
  'switch-off',
  'switch-knob-off',
  'switch-knob-on',
] as const;

/** One style's settings look. */
export type SettingsLook = Readonly<Record<(typeof SETTINGS_TOKENS)[number], string>>;

/** Python's WinUI settings (`settings_style.py`, qfluentwidgets QSS), dark and light. */
const FLUENT: Readonly<Record<'dark' | 'light', SettingsLook>> = {
  dark: {
    page: '#202020',
    text: '#fff',
    card: 'rgb(43,43,43)',
    border: 'rgba(0,0,0,.098)',
    radius: '6px',
    hover: 'rgba(255,255,255,.184)',
    press: 'rgba(255,255,255,.36)',
    'nav-selected': 'rgba(255,255,255,.063)',
    accent: 'var(--sc-accent,var(--sc-brand-fill,#df258e))',
    control: 'rgba(255,255,255,.0605)',
    'control-hover': 'rgba(255,255,255,.0837)',
    'control-press': 'rgba(255,255,255,.0326)',
    'control-border': 'rgba(255,255,255,.053)',
    'control-edge': 'rgba(255,255,255,.08)',
    'control-radius': '5px',
    'field-edge': 'rgba(255,255,255,.5442)',
    'field-focus': 'rgba(30,30,30,.7)',
    disabled: 'rgba(255,255,255,.3628)',
    'switch-off': 'rgba(255,255,255,.6)',
    'switch-knob-off': 'rgba(255,255,255,.79)',
    'switch-knob-on': '#000',
  },
  light: {
    page: '#f3f3f3',
    text: '#000',
    card: 'rgba(255,255,255,.702)',
    border: 'rgba(0,0,0,.059)',
    radius: '6px',
    hover: 'rgba(0,0,0,.184)',
    press: 'rgba(0,0,0,.36)',
    'nav-selected': 'rgba(0,0,0,.035)',
    accent: 'var(--sc-accent,var(--sc-brand-fill,#df258e))',
    control: 'rgba(255,255,255,.7)',
    'control-hover': 'rgba(249,249,249,.5)',
    'control-press': 'rgba(249,249,249,.3)',
    'control-border': 'rgba(0,0,0,.073)',
    'control-edge': 'rgba(0,0,0,.183)',
    'control-radius': '5px',
    'field-edge': 'rgba(0,0,0,.392)',
    'field-focus': '#fff',
    disabled: 'rgba(0,0,0,.36)',
    'switch-off': 'rgba(0,0,0,.52)',
    'switch-knob-off': 'rgba(0,0,0,.61)',
    'switch-knob-on': '#fff',
  },
};

/** A styled look built on the style's own theme tokens, which the workspace root carries. */
function themed(radius: string): SettingsLook {
  return {
    page: 'var(--sc-surface)',
    text: 'var(--sc-text)',
    card: 'var(--sc-surface-secondary)',
    border: 'var(--sc-border-subtle)',
    radius,
    hover: 'var(--sc-subtle-hover)',
    press: 'var(--sc-subtle-selected)',
    'nav-selected': 'var(--sc-subtle-selected)',
    accent: 'var(--sc-brand-fill)',
    control: 'var(--sc-button-fill)',
    'control-hover': 'var(--sc-button-hover-fill)',
    'control-press': 'var(--sc-button-pressed-fill)',
    'control-border': 'var(--sc-border)',
    'control-edge': 'var(--sc-border)',
    'control-radius': 'var(--sc-control-radius)',
    'field-edge': 'var(--sc-field-bottom-stroke)',
    'field-focus': 'var(--sc-field-focus-fill)',
    disabled: 'var(--sc-text-disabled)',
    'switch-off': 'var(--sc-border-accessible)',
    'switch-knob-off': 'var(--sc-text-secondary)',
    'switch-knob-on': 'var(--sc-text-on-accent)',
  };
}

/** ComfyUI's own palette, for the Comfy style. */
const COMFY: SettingsLook = {
  page: 'var(--comfy-menu-bg,#202020)',
  text: 'var(--fg-color,#ddd)',
  card: 'color-mix(in srgb,var(--fg-color,#fff) 4%,transparent)',
  border: 'color-mix(in srgb,var(--fg-color,#fff) 8%,transparent)',
  radius: '6px',
  hover: 'color-mix(in srgb,var(--fg-color,#fff) 7%,transparent)',
  press: 'color-mix(in srgb,var(--fg-color,#fff) 12%,transparent)',
  'nav-selected': 'color-mix(in srgb,var(--fg-color,#fff) 7%,transparent)',
  accent: 'var(--p-primary-color,#df258e)',
  control: 'var(--comfy-input-bg,#222)',
  'control-hover': 'color-mix(in srgb,var(--comfy-input-bg,#222) 85%,var(--fg-color,#fff))',
  'control-press': 'var(--comfy-input-bg,#222)',
  'control-border': 'var(--border-color,#4e4e4e)',
  'control-edge': 'var(--border-color,#4e4e4e)',
  'control-radius': '5px',
  'field-edge': 'var(--border-color,#4e4e4e)',
  'field-focus': 'var(--comfy-input-bg,#222)',
  disabled: 'color-mix(in srgb,var(--fg-color,#fff) 40%,transparent)',
  'switch-off': 'color-mix(in srgb,var(--fg-color,#fff) 60%,transparent)',
  'switch-knob-off': 'color-mix(in srgb,var(--fg-color,#fff) 80%,transparent)',
  'switch-knob-on': '#fff',
};

/** Where the workspace is presented: the SugarSubstitute frame, or a page of plain ComfyUI. */
export type SettingsHost = 'frame' | 'page';

/**
 * The settings look of one visual style, palette and host. Fluent in the SugarSubstitute frame
 * is the Python application's settings as measured; Fluent in plain ComfyUI keeps the same cards
 * and controls on Fluent's own theme tokens.
 */
export function settingsLook(style: VisualStyle, dark: boolean, host: SettingsHost): SettingsLook {
  if (style === 'fluent') return host === 'frame' ? FLUENT[dark ? 'dark' : 'light'] : themed('6px');
  if (style === 'photopro') return themed('0');
  if (style === 'gimp') return themed('3px');
  if (style === 'spectrum') return themed('8px');
  return COMFY;
}

/** Every look as rules keyed by the workspace root's `data-look`, `data-mode` and `data-scheme`. */
export function settingsLookCss(): string {
  const styles: readonly VisualStyle[] = ['fluent', 'photopro', 'gimp', 'spectrum', 'comfy'];
  return styles
    .flatMap((style) =>
      (['frame', 'page'] as const).flatMap((host) =>
        (['dark', 'light'] as const).map((scheme) => {
          const look = settingsLook(style, scheme === 'dark', host);
          const declarations = SETTINGS_TOKENS.map(
            (token) => `--sc-sw-${token}:${look[token]};`,
          ).join('');
          return `.sc-sw[data-look="${style}"][data-mode="${host}"][data-scheme="${scheme}"] { ${declarations} }`;
        }),
      ),
    )
    .join('\n');
}
