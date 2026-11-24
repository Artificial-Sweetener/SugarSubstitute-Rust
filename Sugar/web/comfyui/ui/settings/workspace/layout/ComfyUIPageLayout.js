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
/** ComfyUI: the connection rows that have an owner today. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const comfyuiPage = {
    id: 'comfyui',
    title: 'ComfyUI',
    subtitle: 'Connection, setup, and Python environment',
    icon: 'plug-connected-settings',
    searchable: true,
    sections: [
        {
            id: 'connection',
            title: 'Connection',
            rows: [
                row('comfyui.connection.model_folder', 'Model folder', 'Model folder of this ComfyUI installation. A change applies after ComfyUI restarts.', 'folder-open', 720),
            ],
        },
    ],
};
