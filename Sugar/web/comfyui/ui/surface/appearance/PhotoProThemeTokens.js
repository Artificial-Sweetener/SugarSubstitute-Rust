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
/** Translate Photoshop's four interface brightness levels into shared semantic roles. */
import { themeTokenDeclarations } from './ThemeTokens.js';
const palettes = {
    black: {
        canvas: '#151515',
        toolSelectedFill: '#181818',
        toolSelectedBorder: '#515151',
        toolForeground: '#d2d2d2',
        tab: '#1d1d1d',
        tabHover: '#303030',
        tabCloseHoverBorder: '#515151',
        tabCloseHoverText: '#b8b8b8',
        tabText: '#aaa',
        tabSelectedText: '#ececec',
        tabBorder: '#111111',
        surface: '#242424',
        hover: '#303030',
        pressed: '#181818',
        secondary: '#1d1d1d',
        tertiary: '#292929',
        border: '#111111',
        borderSubtle: '#303030',
        field: '#1b1b1b',
        fieldStroke: '#515151',
        text: '#ececec',
        textSecondary: '#d2d2d2',
        textMuted: '#b8b8b8',
        textSubtle: '#aaa',
        textDisabled: '#777',
        rangeTrack: '#595959',
        rangeThumb: '#e6e6e6',
        scrollTrack: '#161616',
        scrollThumb: '#383838',
        scrollArrow: '#7a7a7a',
    },
    'dark-gray': {
        canvas: '#202020',
        toolSelectedFill: '#1f1f1f',
        toolSelectedBorder: '#3e3e3e',
        toolForeground: '#d6d6d6',
        tab: '#262626',
        tabHover: '#2e2e2e',
        tabCloseHoverBorder: '#474747',
        tabCloseHoverText: '#808080',
        tabText: '#878787',
        tabSelectedText: '#e1e1e1',
        tabBorder: '#212121',
        surface: '#323232',
        hover: '#404040',
        pressed: '#282828',
        secondary: '#2b2b2b',
        tertiary: '#383838',
        border: '#1f1f1f',
        borderSubtle: '#303030',
        field: '#292929',
        fieldStroke: '#555555',
        text: '#e9e9e9',
        textSecondary: '#d4d4d4',
        textMuted: '#bcbcbc',
        textSubtle: '#aaa',
        textDisabled: '#777',
        rangeTrack: '#666666',
        rangeThumb: '#e6e6e6',
        scrollTrack: '#292929',
        scrollThumb: '#474747',
        scrollArrow: '#797979',
    },
    'medium-gray': {
        canvas: '#282828',
        toolSelectedFill: '#383838',
        toolSelectedBorder: '#666666',
        toolForeground: '#d8d8d8',
        tab: '#424242',
        tabHover: '#5e5e5e',
        tabCloseHoverBorder: '#666666',
        tabCloseHoverText: '#bcbcbc',
        tabText: '#b0b0b0',
        tabSelectedText: '#eaeaea',
        tabBorder: '#393939',
        surface: '#535353',
        hover: '#5e5e5e',
        pressed: '#383838',
        secondary: '#424242',
        tertiary: '#4d4d4d',
        border: '#393939',
        borderSubtle: '#444444',
        field: '#454545',
        fieldStroke: '#666666',
        text: '#eaeaea',
        textSecondary: '#d8d8d8',
        textMuted: '#bcbcbc',
        textSubtle: '#b0b0b0',
        textDisabled: '#7c7c7c',
        rangeTrack: '#757575',
        rangeThumb: '#e6e6e6',
        scrollTrack: '#333333',
        scrollThumb: '#555555',
        scrollArrow: '#999999',
    },
    'light-gray': {
        canvas: '#a6a6a6',
        toolSelectedFill: '#c1c1c1',
        toolSelectedBorder: '#9b9b9b',
        toolForeground: '#333333',
        tab: '#c9c9c9',
        tabHover: '#e2e2e2',
        tabCloseHoverBorder: '#9b9b9b',
        tabCloseHoverText: '#555555',
        tabText: '#666666',
        tabSelectedText: '#222222',
        tabBorder: '#8d8d8d',
        surface: '#d6d6d6',
        hover: '#e2e2e2',
        pressed: '#c1c1c1',
        secondary: '#c9c9c9',
        tertiary: '#d0d0d0',
        border: '#8d8d8d',
        borderSubtle: '#bcbcbc',
        field: '#ededed',
        fieldStroke: '#9b9b9b',
        text: '#222222',
        textSecondary: '#333333',
        textMuted: '#555555',
        textSubtle: '#666666',
        textDisabled: '#858585',
        rangeTrack: '#898989',
        rangeThumb: '#f7f7f7',
        scrollTrack: '#b8b8b8',
        scrollThumb: '#d1d1d1',
        scrollArrow: '#555555',
    },
};
/** Accept only Photoshop's documented desktop interface themes. */
export function isPhotoProPalette(value) {
    return (value === 'black' || value === 'dark-gray' || value === 'medium-gray' || value === 'light-gray');
}
/** Keep medium gray as the existing PhotoPro appearance when no palette is selected. */
export function photoProTokenDeclarations(palette = 'medium-gray') {
    const colors = palettes[palette];
    const tokens = {
        text: colors.text,
        'text-secondary': colors.textSecondary,
        'text-muted': colors.textMuted,
        'text-subtle': colors.textSubtle,
        'text-disabled': colors.textDisabled,
        'text-on-accent': '#ffffff',
        surface: colors.surface,
        'surface-hover': colors.hover,
        'surface-pressed': colors.pressed,
        'surface-secondary': colors.secondary,
        'surface-tertiary': colors.tertiary,
        'surface-disabled': colors.tertiary,
        border: colors.border,
        'border-hover': colors.fieldStroke,
        'border-subtle': colors.borderSubtle,
        'border-accessible': colors.fieldStroke,
        'border-accessible-hover': colors.fieldStroke,
        'border-disabled': colors.hover,
        'brand-fill': 'var(--sc-accent-color,#2c8be8)',
        'brand-fill-hover': 'var(--sc-accent-hover,#459cf0)',
        'brand-fill-pressed': 'var(--sc-accent-pressed,#2372c0)',
        'brand-text': 'var(--sc-accent-indicator,#2c8be8)',
        'brand-border': 'var(--sc-accent-indicator,#2c8be8)',
        'brand-indicator': 'var(--sc-accent-indicator,#2c8be8)',
        'brand-indicator-hover': 'var(--sc-accent-hover,#459cf0)',
        'danger-text': palette === 'light-gray' ? '#b00020' : '#ff9292',
        'focus-ring': 'var(--sc-accent-indicator,#2c8be8)',
        'subtle-hover': colors.hover,
        'subtle-selected': colors.tertiary,
        'font-family': '"PhotoPro Source Sans 3", "Segoe UI", sans-serif',
        'font-size': '12px',
        'line-height': '1.3',
        'font-weight': '400',
        'font-weight-strong': '600',
        'control-radius': '3px',
        'field-radius': '2px',
        'field-border-width': '1px',
        'field-fill': colors.field,
        'field-hover-fill': 'var(--sc-field-fill)',
        'field-focus-fill': 'var(--sc-field-fill)',
        'field-disabled-fill': 'var(--sc-surface-disabled)',
        'field-stroke': colors.fieldStroke,
        'field-bottom-stroke': colors.fieldStroke,
        'field-hover-stroke': colors.textMuted,
        'field-hover-bottom-stroke': colors.textMuted,
        'field-focus-stroke': 'var(--sc-focus-ring)',
        'field-focus-bottom-stroke': 'var(--sc-focus-ring)',
        'field-focus-shadow': 'none',
        'field-focus-outline': '1px solid var(--sc-focus-ring)',
        'field-focus-outline-offset': '1px',
        'field-underline-display': 'none',
        'button-radius': '3px',
        'button-border-width': '1px',
        'button-bottom-stroke': colors.border,
        'button-fill': colors.field,
        'button-hover-fill': colors.hover,
        'button-pressed-fill': colors.pressed,
        'shadow-small': '0 1px 3px #0005',
        'shadow-medium': '0 4px 12px #0008',
        'shadow-large': '0 8px 24px #0009',
        'motion-faster': '80ms',
        'motion-fast': '120ms',
        'motion-duration': '150ms',
        'motion-ease': 'ease-out',
        'motion-decelerate': 'ease-out',
    };
    return `--sc-photopro-pasteboard:${colors.canvas};--sc-photopro-field:${colors.field};--sc-photopro-tool-selected-fill:${colors.toolSelectedFill};--sc-photopro-tool-selected-border:${colors.toolSelectedBorder};--sc-photopro-tool-foreground:${colors.toolForeground};--sc-photopro-tool-hover-border:${colors.tabCloseHoverBorder};--sc-photopro-tool-hover-fill:${colors.field};--sc-photopro-tab:${colors.tab};--sc-photopro-tab-hover:${colors.tabHover};--sc-photopro-tab-close-hover-border:${colors.tabCloseHoverBorder};--sc-photopro-tab-close-hover-text:${colors.tabCloseHoverText};--sc-photopro-tab-text:${colors.tabText};--sc-photopro-tab-selected-text:${colors.tabSelectedText};--sc-photopro-tab-border:${colors.tabBorder};--sc-photopro-list:${colors.tertiary};--sc-photopro-range-track:${colors.rangeTrack};--sc-photopro-range-thumb:${colors.rangeThumb};${themeTokenDeclarations(tokens)}`;
}
/** Keep overlay scrollbar colors on the document root when its layer sits outside a card. */
export function photoProScrollbarTokenDeclarations(palette = 'medium-gray') {
    const colors = palettes[palette];
    return `--sc-photopro-scroll-track:${colors.scrollTrack};--sc-photopro-scroll-thumb:${colors.scrollThumb};--sc-photopro-scroll-arrow:${colors.scrollArrow};--sc-photopro-scroll-border:${colors.border};`;
}
/** Publish document-level surfaces outside themed cards from the same palette. */
function photoProDocumentTokens(palette = 'medium-gray') {
    return `${photoProScrollbarTokenDeclarations(palette)}--sc-photopro-workspace-fill:${palettes[palette].surface};`;
}
/** Update the same tokens on every PhotoPro surface as a palette preference changes. */
export function photoProPaletteCss() {
    return (`html[data-sc-visual-style="photopro"]{${photoProDocumentTokens()}}\n` +
        Object.keys(palettes)
            .filter((palette) => palette !== 'medium-gray')
            .map((palette) => `html[data-sc-visual-style="photopro"][data-sc-photopro-palette="${palette}"]{${photoProDocumentTokens(palette)}}\nhtml[data-sc-visual-style="photopro"][data-sc-photopro-palette="${palette}"] :is([data-sc-style-theme-root],[data-sc-theme="photopro"]){${photoProTokenDeclarations(palette)}}`)
            .join('\n'));
}
