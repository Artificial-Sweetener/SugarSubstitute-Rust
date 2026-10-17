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
/** Define semantic presentation roles shared by every visual-style provider. */
export interface ThemeTokens {
  readonly text: string;
  readonly 'text-secondary': string;
  readonly 'text-muted': string;
  readonly 'text-subtle': string;
  readonly 'text-disabled': string;
  readonly 'text-on-accent': string;
  readonly surface: string;
  readonly 'surface-hover': string;
  readonly 'surface-pressed': string;
  readonly 'surface-secondary': string;
  readonly 'surface-tertiary': string;
  readonly 'surface-disabled': string;
  readonly border: string;
  readonly 'border-hover': string;
  readonly 'border-subtle': string;
  readonly 'border-accessible': string;
  readonly 'border-accessible-hover': string;
  readonly 'border-disabled': string;
  readonly 'brand-fill': string;
  readonly 'brand-fill-hover': string;
  readonly 'brand-fill-pressed': string;
  readonly 'brand-text': string;
  readonly 'brand-border': string;
  readonly 'brand-indicator': string;
  readonly 'brand-indicator-hover': string;
  readonly 'danger-text': string;
  readonly 'focus-ring': string;
  readonly 'subtle-hover': string;
  readonly 'subtle-selected': string;
  readonly 'font-family': string;
  readonly 'font-size': string;
  readonly 'font-weight': string;
  readonly 'font-weight-strong': string;
  readonly 'control-radius': string;
  readonly 'shadow-small': string;
  readonly 'shadow-medium': string;
  readonly 'shadow-large': string;
  readonly 'motion-faster': string;
  readonly 'motion-fast': string;
  readonly 'motion-duration': string;
  readonly 'motion-ease': string;
  readonly 'motion-decelerate': string;
}

/** Serialize trusted provider values under the product's own CSS namespace. */
export function themeTokenDeclarations(tokens: ThemeTokens): string {
  return Object.entries(tokens)
    .map(([role, value]) => `--sc-${role}:${value};`)
    .join('');
}
