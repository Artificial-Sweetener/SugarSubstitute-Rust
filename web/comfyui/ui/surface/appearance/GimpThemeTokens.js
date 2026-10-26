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
/** Map GIMP Default theme roles to SugarCubes; see third_party/manifest.toml for provenance. */
import { themeTokenDeclarations } from './ThemeTokens.js';
/** Keep upstream color meanings intact rather than substituting another provider's palette. */
export function gimpPalette(dark) {
    return dark
        ? {
            foreground: '#dcdcdc',
            background: '#3c3c3c',
            border: '#1a1a1a',
            dimmed: '#9b9b9b',
            disabled: '#919191',
            hover: '#646464',
            widget: '#2a2a2a',
            selected: '#1e1e1e',
            extreme: '#242424',
            extremeSelected: '#373737',
            strongBorder: '#282828',
            strongerBorder: '#4b4b4b',
            edgeBorder: '#1a1a1a',
            scrollbar: '#8c8c8c',
            trough: '#3c3c3c',
            ruler: 'rgba(35,35,35,0.3)',
        }
        : {
            foreground: '#000000',
            background: '#ebebeb',
            border: '#dcdcdc',
            dimmed: '#646464',
            disabled: '#6e6e6e',
            hover: '#fafafa',
            widget: '#dcdcdc',
            selected: '#fafafa',
            extreme: '#fafafa',
            extremeSelected: '#dcdcdc',
            strongBorder: '#c8c8c8',
            strongerBorder: '#b9b9b9',
            edgeBorder: '#b9b9b9',
            scrollbar: '#6e6e6e',
            trough: '#fafafa',
            ruler: 'rgba(220,220,220,0.3)',
        };
}
/** Translate GTK entry, button, notebook and view materials into the complete semantic contract. */
export function gimpThemeTokens(dark) {
    const p = gimpPalette(dark);
    return {
        text: p.foreground,
        'text-secondary': p.foreground,
        'text-muted': p.dimmed,
        'text-subtle': p.dimmed,
        'text-disabled': p.disabled,
        'text-on-accent': p.background,
        surface: p.background,
        'surface-hover': p.hover,
        'surface-pressed': p.selected,
        'surface-secondary': p.widget,
        'surface-tertiary': p.extreme,
        'surface-disabled': p.background,
        border: p.border,
        'border-hover': p.strongerBorder,
        'border-subtle': p.strongBorder,
        'border-accessible': p.strongerBorder,
        'border-accessible-hover': p.foreground,
        'border-disabled': p.strongBorder,
        'brand-fill': p.foreground,
        'brand-fill-hover': p.dimmed,
        'brand-fill-pressed': p.hover,
        'brand-text': p.foreground,
        'brand-border': p.strongerBorder,
        'brand-indicator': p.foreground,
        'brand-indicator-hover': p.dimmed,
        'danger-text': dark ? '#ff8989' : '#a02020',
        'focus-ring': p.foreground,
        'subtle-hover': p.hover,
        'subtle-selected': p.selected,
        'font-family': '"Segoe UI", "Noto Sans", sans-serif',
        'font-size': '12px',
        'line-height': '18px',
        'font-weight': '400',
        'font-weight-strong': '600',
        'control-radius': '3px',
        'field-radius': '3px',
        'field-border-width': '1px',
        'field-fill': p.widget,
        'field-stroke': p.strongerBorder,
        'field-bottom-stroke': p.strongerBorder,
        'field-hover-stroke': p.foreground,
        'field-hover-bottom-stroke': p.foreground,
        'field-focus-stroke': p.strongerBorder,
        'field-focus-bottom-stroke': p.strongerBorder,
        'field-focus-shadow': 'none',
        'field-focus-outline': `1px dotted ${p.foreground}`,
        'field-focus-outline-offset': '-3px',
        'field-underline-display': 'none',
        'button-radius': '3px',
        'button-border-width': '1px',
        'button-bottom-stroke': p.strongBorder,
        'button-fill': p.widget,
        'button-hover-fill': p.hover,
        'button-pressed-fill': p.selected,
        'shadow-small': 'none',
        'shadow-medium': '0 2px 5px #0005',
        'shadow-large': '0 4px 12px #0005',
        'motion-faster': '0ms',
        'motion-fast': '0ms',
        'motion-duration': '0ms',
        'motion-ease': 'linear',
        'motion-decelerate': 'linear',
    };
}
/** Publish upstream roles alongside the shared roles for GTK-specific control painting. */
export function gimpTokenDeclarations(dark) {
    const colors = gimpPalette(dark);
    const palette = Object.entries(colors)
        .map(([role, value]) => `--sc-gimp-${role}:${value};`)
        .join('');
    return (themeTokenDeclarations(gimpThemeTokens(dark)) +
        palette +
        `--sc-gimp-combo-highlight:${dark ? colors.extremeSelected : colors.extreme};` +
        '--sc-control-height:26px;--sc-number-stepper-width:22px;--sc-spinner-stepper-width:calc(2 * var(--sc-number-stepper-width));' +
        `--sc-gimp-spin-scale-entry:${dark ? colors.widget : colors.extreme};` +
        `--sc-gimp-spin-scale-fill:${dark ? colors.hover : colors.widget};`);
}
