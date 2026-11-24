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
/** Appearance: style, system colors and layout. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const appearancePage = {
    id: 'appearance',
    title: 'Appearance',
    subtitle: 'Visual customization.',
    icon: 'paint-brush',
    searchable: true,
    sections: [
        {
            id: 'style',
            title: 'Style',
            rows: [
                row('appearance.style.visual', 'Visual style', 'Choose Comfy, Fluent, Spectrum 2, PhotoPro, or GIMP for node cards, masonry, and SugarCubes surfaces.', 'paint-brush', 640, ['fluent', 'spectrum', 'photopro', 'gimp', 'theme']),
                row('appearance.style.photopro_palette', 'PhotoPro color theme', 'Choose one of Photoshop desktop’s four interface brightness levels.', 'paint-brush', 640),
            ],
        },
        {
            id: 'system_colors',
            title: 'System colors',
            rows: [
                row('appearance.system_colors.accent', 'Accent color', 'The shared accent of selections and highlights. Hosts can override it with --sugarcubes-accent on the document root.', 'color', 640),
            ],
        },
        {
            id: 'layout',
            title: 'Layout',
            rows: [
                row('appearance.layout.substitute_frame', 'Substitute frame', 'Use Substitute workflow tabs, application orb and transparent Cube stack; hide the Comfy sidebar.', 'window-brush'),
                row('appearance.layout.plugin_mode', 'Plugin Mode', 'Show global overrides as an always-expanded node card above the cubes.', 'window-brush'),
                row('appearance.layout.compact_masonry', 'Compact masonry', 'Pack node cards edge to edge with square corners in every visual style.', 'window-brush'),
                row('appearance.layout.cube_stack', 'Show Cube stack', 'Show the Cube stack beside masonry.', 'window-brush'),
                row('appearance.layout.compact_stack', 'Compact Cube stack', 'Show the Cube stack as a narrow rail.', 'window-brush'),
            ],
        },
    ],
};
