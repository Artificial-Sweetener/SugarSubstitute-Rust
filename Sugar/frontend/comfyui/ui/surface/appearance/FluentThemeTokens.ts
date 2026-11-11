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
/** Translate Microsoft's theme vocabulary into SugarCubes semantic presentation roles. */
import type { ThemeTokens } from './ThemeTokens.js';

/**
 * Supply Fluent's input fill ramp, which Microsoft's web token set does not publish.
 *
 * WinUI's ControlFillColor Default/Secondary/InputActive/Disabled values, also used by the
 * qfluentwidgets LineEdit, SpinBox and ComboBox styles: a focused field darkens in the dark theme
 * and becomes solid white in the light theme. The fills are translucent so they compose over any
 * card material.
 */
const fieldFills = {
  dark: {
    'field-fill': 'rgba(255,255,255,0.0605)',
    'field-hover-fill': 'rgba(255,255,255,0.0837)',
    'field-focus-fill': 'rgba(30,30,30,0.7)',
    'field-disabled-fill': 'rgba(255,255,255,0.0419)',
  },
  light: {
    'field-fill': 'rgba(255,255,255,0.7)',
    'field-hover-fill': 'rgba(249,249,249,0.5)',
    'field-focus-fill': '#ffffff',
    'field-disabled-fill': 'rgba(249,249,249,0.3)',
  },
} as const;

/** Require every presentation role to be supplied by Microsoft's trusted theme for one scheme. */
export function fluentThemeTokens(
  theme: Readonly<Record<string, string | number | undefined>>,
  dark: boolean,
): ThemeTokens {
  return {
    text: String(theme.colorNeutralForeground1 ?? ''),
    'text-secondary': String(theme.colorNeutralForeground2 ?? ''),
    'text-muted': String(theme.colorNeutralForeground3 ?? ''),
    'text-subtle': String(theme.colorNeutralForeground4 ?? ''),
    'text-disabled': String(theme.colorNeutralForegroundDisabled ?? ''),
    'text-on-accent': String(theme.colorNeutralForegroundOnBrand ?? ''),
    surface: String(theme.colorNeutralBackground1 ?? ''),
    'surface-hover': String(theme.colorNeutralBackground1Hover ?? ''),
    'surface-pressed': String(theme.colorNeutralBackground1Pressed ?? ''),
    'surface-secondary': String(theme.colorNeutralBackground2 ?? ''),
    'surface-tertiary': String(theme.colorNeutralBackground3 ?? ''),
    'surface-disabled': String(theme.colorNeutralBackgroundDisabled ?? ''),
    border: String(theme.colorNeutralStroke1 ?? ''),
    'border-hover': String(theme.colorNeutralStroke1Hover ?? ''),
    'border-subtle': String(theme.colorNeutralStroke2 ?? ''),
    'border-accessible': String(theme.colorNeutralStrokeAccessible ?? ''),
    'border-accessible-hover': String(theme.colorNeutralStrokeAccessibleHover ?? ''),
    'border-disabled': String(theme.colorNeutralStrokeDisabled ?? ''),
    'brand-fill': String(theme.colorBrandBackground ?? ''),
    'brand-fill-hover': String(theme.colorBrandBackgroundHover ?? ''),
    'brand-fill-pressed': String(theme.colorBrandBackgroundPressed ?? ''),
    'brand-text': String(theme.colorBrandForeground1 ?? ''),
    'brand-border': String(theme.colorBrandStroke1 ?? ''),
    'brand-indicator': String(theme.colorCompoundBrandBackground ?? ''),
    'brand-indicator-hover': String(theme.colorCompoundBrandBackgroundHover ?? ''),
    'danger-text': String(theme.colorPaletteRedForeground1 ?? ''),
    'focus-ring': String(theme.colorStrokeFocus2 ?? ''),
    'subtle-hover': String(theme.colorSubtleBackgroundHover ?? ''),
    'subtle-selected': String(theme.colorSubtleBackgroundSelected ?? ''),
    'font-family': String(theme.fontFamilyBase ?? ''),
    'font-size': String(theme.fontSizeBase300 ?? ''),
    'line-height': String(theme.lineHeightBase300 ?? ''),
    'font-weight': String(theme.fontWeightRegular ?? ''),
    'font-weight-strong': String(theme.fontWeightSemibold ?? ''),
    'control-radius': String(theme.borderRadiusMedium ?? ''),
    'field-radius': 'var(--sc-control-radius)',
    'field-border-width': '1px',
    ...fieldFills[dark ? 'dark' : 'light'],
    'field-stroke': 'var(--sc-border)',
    'field-bottom-stroke': 'var(--sc-border-accessible)',
    'field-hover-stroke': 'var(--sc-border-hover)',
    'field-hover-bottom-stroke': 'var(--sc-border-accessible-hover)',
    'field-focus-stroke': 'var(--sc-border)',
    'field-focus-bottom-stroke': 'var(--sc-brand-border)',
    'field-focus-shadow': 'inset 0 -1px var(--sc-brand-border)',
    'field-focus-outline': 'none',
    'field-focus-outline-offset': '0px',
    'field-underline-display': 'block',
    'button-radius': 'var(--sc-control-radius)',
    'button-border-width': '1px',
    'button-bottom-stroke': 'var(--sc-border-accessible)',
    'button-fill': 'var(--sc-surface)',
    'button-hover-fill': 'var(--sc-surface-hover)',
    'button-pressed-fill': 'var(--sc-surface-pressed)',
    'shadow-small': String(theme.shadow4 ?? ''),
    'shadow-medium': String(theme.shadow16 ?? ''),
    'shadow-large': String(theme.shadow64 ?? ''),
    'motion-faster': String(theme.durationFaster ?? ''),
    'motion-fast': String(theme.durationFast ?? ''),
    'motion-duration': String(theme.durationNormal ?? ''),
    'motion-ease': String(theme.curveEasyEase ?? ''),
    'motion-decelerate': String(theme.curveDecelerateMid ?? ''),
  };
}
