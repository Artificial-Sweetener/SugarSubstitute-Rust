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
/** Adapt Cube authoring to Comfy's command contract. */
export const CONVERT_SELECTION_TO_CUBE_COMMAND = 'SugarCubes.Graph.ConvertSelectionToCube';
/** Own the host-visible entry points for converting a selection into a Cube. */
export class CubeAuthoringHostCommands {
    #createCube;
    constructor(createCube) {
        this.#createCube = createCube;
    }
    /** Build the command descriptors registered through Comfy's extension API. */
    get commands() {
        return [
            {
                id: CONVERT_SELECTION_TO_CUBE_COMMAND,
                icon: 'mdi mdi-cube-outline',
                label: 'Convert Selection to SugarCube',
                function: this.#createCube,
            },
        ];
    }
}
