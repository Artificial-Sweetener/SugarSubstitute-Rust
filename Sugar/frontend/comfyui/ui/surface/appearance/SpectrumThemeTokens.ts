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
/** Translate Adobe Spectrum 2 foundations into shared SugarCubes presentation roles. */
import { themeTokenDeclarations, type ThemeTokens } from './ThemeTokens.js';

/** Extract only root token declarations from trusted packaged Adobe styles, excluding typography rules. */
export function spectrumRootTokens(cssText: string): string {
  return [...cssText.matchAll(/(?::root,:host|:host,:root)\{([^{}]*)\}/g)]
    .map((match) => match[1] ?? '')
    .join(';');
}

/** Resolve semantic roles through Adobe's light/dark-aware Spectrum 2 design tokens. */
export function spectrumSemanticTokens(): string {
  const tokens: ThemeTokens = {
    text: 'var(--spectrum-gray-900)',
    'text-secondary': 'var(--spectrum-gray-800)',
    'text-muted': 'var(--spectrum-gray-700)',
    'text-subtle': 'var(--spectrum-gray-600)',
    'text-disabled': 'var(--spectrum-gray-500)',
    'text-on-accent': 'var(--sc-accent-foreground,#fff)',
    surface: 'var(--spectrum-gray-75)',
    'surface-hover': 'var(--spectrum-gray-200)',
    'surface-pressed': 'var(--spectrum-gray-300)',
    'surface-secondary': 'var(--spectrum-gray-50)',
    'surface-tertiary': 'var(--spectrum-gray-100)',
    'surface-disabled': 'var(--spectrum-gray-200)',
    border: 'var(--spectrum-gray-400)',
    'border-hover': 'var(--spectrum-gray-500)',
    'border-subtle': 'var(--spectrum-gray-300)',
    'border-accessible': 'var(--spectrum-gray-500)',
    'border-accessible-hover': 'var(--spectrum-gray-600)',
    'border-disabled': 'var(--spectrum-gray-300)',
    'brand-fill': 'var(--sc-accent-color,var(--spectrum-blue-900))',
    'brand-fill-hover': 'var(--sc-accent-hover,var(--spectrum-blue-1000))',
    'brand-fill-pressed': 'var(--sc-accent-pressed,var(--spectrum-blue-1100))',
    'brand-text': 'var(--sc-accent-indicator,var(--spectrum-blue-900))',
    'brand-border': 'var(--sc-accent-indicator,var(--spectrum-blue-900))',
    'brand-indicator': 'var(--sc-accent-indicator,var(--spectrum-blue-900))',
    'brand-indicator-hover': 'var(--sc-accent-hover,var(--spectrum-blue-1000))',
    'danger-text': 'var(--spectrum-red-900)',
    'focus-ring': 'var(--sc-accent-indicator,var(--spectrum-blue-900))',
    'subtle-hover': 'var(--spectrum-gray-200)',
    'subtle-selected': 'var(--spectrum-gray-300)',
    'font-family': 'var(--spectrum-sans-font-family-stack,"Segoe UI",sans-serif)',
    'font-size': 'var(--spectrum-font-size-100,14px)',
    'line-height': 'var(--spectrum-line-height-100)',
    'font-weight': '400',
    'font-weight-strong': '600',
    'control-radius': 'var(--spectrum-corner-radius-100,8px)',
    'field-radius': 'var(--system-textfield-corner-radius)',
    'field-border-width': 'var(--system-textfield-border-width)',
    'field-fill': 'var(--system-textfield-background-color)',
    'field-hover-fill': 'var(--sc-field-fill)',
    'field-focus-fill': 'var(--sc-field-fill)',
    'field-disabled-fill': 'var(--sc-surface-disabled)',
    'field-stroke': 'var(--system-textfield-border-color)',
    'field-bottom-stroke': 'var(--system-textfield-border-color)',
    'field-hover-stroke': 'var(--system-textfield-border-color-hover)',
    'field-hover-bottom-stroke': 'var(--system-textfield-border-color-hover)',
    'field-focus-stroke': 'var(--system-textfield-border-color-focus)',
    'field-focus-bottom-stroke': 'var(--system-textfield-border-color-focus)',
    'field-focus-shadow': 'none',
    'field-focus-outline': 'var(--spectrum-focus-indicator-thickness) solid var(--sc-focus-ring)',
    'field-focus-outline-offset': 'var(--spectrum-focus-indicator-gap)',
    'field-underline-display': 'none',
    'button-radius': '999px',
    'button-border-width': 'var(--spectrum-border-width-200)',
    'button-bottom-stroke': 'var(--sc-border)',
    'button-fill': 'var(--system-button-background-color-default)',
    'button-hover-fill': 'var(--system-button-background-color-hover)',
    'button-pressed-fill': 'var(--system-button-background-color-down)',
    'shadow-small': '0 1px 4px #0002',
    'shadow-medium': '0 4px 16px #0003',
    'shadow-large': '0 8px 32px #0004',
    'motion-faster': '100ms',
    'motion-fast': '150ms',
    'motion-duration': '200ms',
    'motion-ease': 'cubic-bezier(.4,0,.2,1)',
    'motion-decelerate': 'cubic-bezier(0,0,.2,1)',
  };
  return themeTokenDeclarations(tokens);
}
