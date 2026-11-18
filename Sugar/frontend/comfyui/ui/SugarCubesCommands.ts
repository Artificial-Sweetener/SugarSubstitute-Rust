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
/** List the commands SugarCubes registers with Comfy and where Comfy's own menu shows them. */
import type { ProjectCommand } from './projects/presentation/ProjectCommands.js';

export interface SugarCubesCommandOwners {
  openCart(): unknown;
  toggleMasonry(): unknown;
  toggleCanvas(): unknown;
  /** The project commands, in menu order. */
  readonly projects: readonly ProjectCommand[];
}

/**
 * Build the extension's `commands` and `menuCommands`.
 *
 * The project commands also sit in Comfy's File menu, so a page without SugarSubstitute's frame
 * offers them where it offers its own open and save.
 */
export function sugarCubesCommandRegistration(owners: SugarCubesCommandOwners): {
  commands: { id: string; label: string; function: () => unknown }[];
  menuCommands: { path: string[]; commands: string[] }[];
} {
  return {
    commands: [
      {
        id: 'SugarCubes.CubeStackCart',
        label: 'Add cubes / edit Cube stack',
        function: () => owners.openCart(),
      },
      {
        id: 'SugarCubes.ToggleMasonry',
        label: 'Toggle workflow masonry',
        function: () => owners.toggleMasonry(),
      },
      {
        id: 'SugarCubes.ToggleOutputCanvas',
        label: 'Toggle output canvas',
        function: () => owners.toggleCanvas(),
      },
      ...owners.projects.map((command) => ({
        id: command.id,
        label: command.label,
        function: () => command.function(),
      })),
    ],
    menuCommands: [{ path: ['File'], commands: owners.projects.map((command) => command.id) }],
  };
}
