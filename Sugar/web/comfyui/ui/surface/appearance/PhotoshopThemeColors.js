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
/** Map Photoshop panel and widget neutrals into Spectrum's shared color roles. */
/**
 * Match Photoshop's darkest/lightest panel palettes while retaining Spectrum control geometry.
 *
 * Reference Photoshop 2026's Required/UXP/host-theme.css and UIColors.txt color values.
 * Host CSS variables take precedence when provided by an embedding surface; standalone
 * Comfy uses the corresponding darkest/lightest colors. These are color mappings, not
 * a Photoshop runtime or a transport for theme changes across iframe boundaries.
 */
export function photoshopColorDeclarations(dark) {
    const colors = {
        '--sc-photoshop-panel': `var(--uxp-host-background-color,${dark ? '#323232' : '#f0f0f0'})`,
        '--sc-photoshop-canvas': `var(--uxp-host-border-color,${dark ? '#292929' : '#d1d1d1'})`,
        '--sc-photoshop-field': dark ? '#2a2a2a' : '#fcfcfc',
        '--sc-photoshop-stroke': `var(--uxp-host-toggle-border-color,${dark ? '#474747' : '#cccccc'})`,
        '--sc-photoshop-hover': `var(--uxp-host-widget-hover-background-color,${dark ? '#3d3d3d' : '#cecece'})`,
        '--sc-photoshop-pressed': `var(--uxp-host-toggle-selected-background-color,${dark ? '#474747' : '#bfbfbf'})`,
        '--spectrum-gray-25': 'var(--sc-photoshop-field)',
        '--spectrum-gray-50': 'var(--sc-photoshop-canvas)',
        '--spectrum-gray-75': 'var(--sc-photoshop-panel)',
        '--spectrum-gray-100': 'var(--sc-photoshop-panel)',
        '--spectrum-gray-200': 'var(--sc-photoshop-hover)',
        '--spectrum-gray-300': 'var(--sc-photoshop-pressed)',
        '--spectrum-gray-400': dark ? '#606060' : '#adadad',
        '--spectrum-gray-500': dark ? '#737373' : '#a8a8a8',
        '--spectrum-gray-600': `var(--uxp-host-text-color-secondary,${dark ? '#9b9b9b' : '#606060'})`,
        '--spectrum-gray-700': dark ? '#adadad' : '#545454',
        '--spectrum-gray-800': `var(--uxp-host-text-button-text-color,${dark ? '#c8c8c8' : '#505050'})`,
        '--spectrum-gray-900': `var(--uxp-host-toggle-text-color,${dark ? '#e1e1e1' : '#2a2a2a'})`,
        '--spectrum-gray-1000': `var(--uxp-host-text-color,${dark ? '#ffffff' : '#292929'})`,
        '--system-card-border-color': 'var(--sc-photoshop-canvas)',
        '--system-textfield-background-color': 'var(--sc-photoshop-field)',
        '--system-textfield-border-color': 'var(--sc-photoshop-stroke)',
        '--system-textfield-border-color-hover': 'var(--spectrum-gray-400)',
        '--system-menu-item-background-color-hover': 'var(--sc-photoshop-hover)',
        '--system-menu-item-background-color-down': 'var(--sc-photoshop-pressed)',
        '--system-menu-item-background-color-key-focus': 'var(--sc-photoshop-hover)',
        '--system-button-background-color-default': dark ? '#292929' : '#fcfcfc',
        '--system-button-background-color-hover': 'var(--sc-photoshop-hover)',
        '--system-button-background-color-down': dark ? '#1f1f1f' : '#bfbfbf',
    };
    return Object.entries(colors)
        .map(([name, color]) => `${name}:${color};`)
        .join('');
}
