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
/** Prompt Editing: the rows that have an owner today. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const promptEditingPage = {
    id: 'prompt_editing',
    title: 'Prompt Editing',
    subtitle: 'Prompt editor behavior and authoring support.',
    icon: 'text-effects-sparkle',
    searchable: true,
    sections: [
        {
            id: 'wildcards',
            title: 'Wildcards',
            rows: [
                row('prompt_editing.wildcards.manage', 'Manage wildcards', 'Wildcards are .txt and .csv files in the sugar/wildcards folder of your ComfyUI user folder. A prompt names one as {colors}, {people/hair}, {csv:people:hair} or {colors|tag}, and each run picks from it with the run’s seed.', 'folder-open', undefined, ['wildcard']),
            ],
        },
    ],
};
