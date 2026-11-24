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
/** Library: Cube Packs, drawn by its own page body, and authoring. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const libraryPage = {
    id: 'library',
    title: 'Library',
    subtitle: 'Cube Packs and readiness',
    icon: 'library',
    searchable: false,
    sections: [
        { id: 'library', title: '', rows: [{ id: 'library.page', title: 'Library' }] },
        {
            id: 'authoring',
            title: 'Authoring',
            rows: [
                row('library.authoring.claimed_owner', 'Claimed GitHub owner', 'Claim one GitHub owner to unlock writes for matching tracked repos. local/... always stays writable.', 'cube-multiple'),
            ],
        },
    ],
};
