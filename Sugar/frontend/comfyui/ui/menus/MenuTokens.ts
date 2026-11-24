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
 * One menu look per visual style, as tokens: the surface, its stroke, corners, padding and shadow,
 * a row's height, padding, corners and state fills, and its text. SugarCubes' own menus are drawn
 * in that look, and in the SugarSubstitute frame ComfyUI's menus read the same tokens, so both
 * follow whichever style is active.
 */

/** The token names, as CSS custom properties. */
export const MENU_TOKENS = [
  'surface',
  'stroke',
  'radius',
  'padding',
  'shadow',
  'row-height',
  'row-padding',
  'row-radius',
  'row-hover',
  'row-pressed',
  'text',
  'text-disabled',
  'separator',
  'font',
] as const;

/** One style's menu look. */
export type MenuLook = Readonly<Record<(typeof MENU_TOKENS)[number], string>>;

/** Each style's menu look, matching how SugarCubes draws its own menus in that style. */
export const MENU_LOOKS: {
  readonly fluent: MenuLook;
  readonly photopro: MenuLook;
  readonly gimp: MenuLook;
} = {
  // Fluent's menu list and menu item, with disabled rows left on the surface as in WinUI.
  fluent: {
    surface: 'var(--colorNeutralBackground1,var(--sc-surface,#292929))',
    stroke: 'var(--colorTransparentStroke,transparent)',
    radius: '4px',
    padding: '4px',
    shadow: 'var(--sc-shadow-medium,0 8px 16px rgba(0,0,0,.28))',
    'row-height': '32px',
    'row-padding': '0 6px',
    'row-radius': '4px',
    'row-hover': 'var(--colorNeutralBackground1Hover,var(--sc-surface-hover))',
    'row-pressed': 'var(--colorNeutralBackground1Pressed,var(--sc-surface-pressed))',
    text: 'var(--colorNeutralForeground2,var(--sc-text))',
    'text-disabled': 'var(--colorNeutralForegroundDisabled,var(--sc-text-disabled))',
    separator: 'var(--colorNeutralStroke2,var(--sc-border-subtle))',
    font: '14px/20px var(--sc-font-family,"Segoe UI",sans-serif)',
  },
  // Photoshop's light, square menus (`photoProMenuCss`).
  photopro: {
    surface: '#f0f0f0',
    stroke: '#8a8a8a',
    radius: '0',
    padding: '2px 0',
    shadow: '1px 2px 5px #0007',
    'row-height': '18px',
    'row-padding': '0 20px',
    'row-radius': '0',
    'row-hover': '#cce8ff',
    'row-pressed': '#cce8ff',
    text: '#111',
    'text-disabled': '#888',
    separator: '#bdbdbd',
    font: '12px/18px Arial,sans-serif',
  },
  // GIMP's menus (`gimpAppearanceCss`).
  gimp: {
    surface: 'var(--sc-gimp-background)',
    stroke: 'var(--sc-gimp-border)',
    radius: '3px',
    padding: '3px',
    shadow: 'var(--sc-shadow-medium)',
    'row-height': '24px',
    'row-padding': '3px 24px',
    'row-radius': '2px',
    'row-hover': 'var(--sc-gimp-selected)',
    'row-pressed': 'var(--sc-gimp-selected)',
    text: 'var(--sc-gimp-foreground)',
    'text-disabled': 'var(--sc-gimp-disabled)',
    separator: 'var(--sc-gimp-border)',
    font: '12px/18px var(--sc-font-family)',
  },
};

/** The styles that have a menu look. */
export type MenuLookStyle = keyof typeof MENU_LOOKS;

/** Whether a visual style has a menu look. */
export function hasMenuLook(style: string): style is MenuLookStyle {
  return Object.prototype.hasOwnProperty.call(MENU_LOOKS, style);
}

/**
 * One style's menu look as declarations. Its values refer to the style's theme tokens, so they
 * must be declared where those tokens are in scope.
 */
export function menuTokenDeclarations(style: MenuLookStyle): string {
  const look = MENU_LOOKS[style];
  return MENU_TOKENS.map((token) => `--sc-menu-${token}:${look[token]};`).join('');
}
