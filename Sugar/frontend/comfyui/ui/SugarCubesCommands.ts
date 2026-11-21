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

/** One view the workspace shows or hides. */
interface Toggled {
  toggle(): unknown;
}

export interface SugarCubesCommandOwners {
  openCart(): unknown;
  /** The workspace views the View commands show or hide. */
  readonly workspace: {
    readonly masonry: Toggled;
    readonly canvas: Toggled;
    readonly console: Toggled;
  };
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
  keybindings: { combo: { key: string; ctrl?: boolean }; commandId: string }[];
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
        function: () => owners.workspace.masonry.toggle(),
      },
      {
        id: 'SugarCubes.ToggleOutputCanvas',
        label: 'Toggle output canvas',
        function: () => owners.workspace.canvas.toggle(),
      },
      {
        id: 'SugarCubes.ToggleConsole',
        label: 'Toggle Console',
        function: () => owners.workspace.console.toggle(),
      },
      ...owners.projects.map((command) => ({
        id: command.id,
        label: command.label,
        function: () => command.function(),
      })),
    ],
    menuCommands: [
      { path: ['File'], commands: owners.projects.map((command) => command.id) },
      { path: ['View'], commands: ['SugarCubes.ToggleConsole'] },
    ],
    // ComfyUI's own console key opens the console wherever it is: SugarCubes' panel inside the
    // SugarSubstitute frame, ComfyUI's Logs panel elsewhere.
    keybindings: [{ combo: { key: '`', ctrl: true }, commandId: 'SugarCubes.ToggleConsole' }],
  };
}
