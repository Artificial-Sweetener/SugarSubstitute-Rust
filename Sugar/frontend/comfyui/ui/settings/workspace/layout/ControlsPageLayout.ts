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
/** Controls: keyboard and graph controls. */
import { row, type SettingsPageLayout } from '../SettingsLayoutRows.js';

/** The page's place in the navigation, its text, and its sections' rows. */
export const controlsPage: SettingsPageLayout = {
  id: 'controls',
  title: 'Controls',
  subtitle: 'Configure keyboard and future input controls.',
  icon: 'cursor-hover',
  searchable: true,
  sections: [
    {
      id: 'keyboard',
      title: 'Keyboard',
      subtitle: 'Assign keyboard controls in ComfyUI’s keybindings.',
      rows: [
        row(
          'controls.keyboard.cube_stack',
          'Add cubes or edit the Cube stack',
          'Open the Cube stack cart.',
          'keyboard',
        ),
        row(
          'controls.keyboard.masonry',
          'Toggle workflow masonry',
          'Switch between the graph and masonry.',
          'keyboard',
        ),
        row(
          'controls.keyboard.output_canvas',
          'Toggle output canvas',
          'Show or hide the output canvas.',
          'keyboard',
        ),
        row('controls.keyboard.console', 'Toggle console', 'Show or hide the console.', 'keyboard'),
      ],
    },
    {
      id: 'graph',
      title: 'Graph',
      rows: [
        row(
          'controls.graph.proximity_links',
          'Proximity links',
          'Nearby compatible cube markers connect automatically during queueing, with an overlay preview.',
          'cursor-hover',
        ),
      ],
    },
  ],
};
