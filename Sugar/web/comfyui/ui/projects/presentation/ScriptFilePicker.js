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
/** Let the person choose a Sugar script in the operating system's own file dialog. */
/**
 * Show the operating system's file dialog for one `.sugar` file and read what was chosen.
 *
 * Resolves `null` when the dialog is dismissed. The input lives only for this one choice, so a
 * second choice of the same file is reported again.
 */
export function pickScriptFile(document) {
    return new Promise((resolve, reject) => {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.sugar';
        input.hidden = true;
        input.addEventListener('cancel', () => {
            input.remove();
            resolve(null);
        });
        input.addEventListener('change', () => {
            const file = input.files?.[0];
            input.remove();
            if (!file) {
                resolve(null);
                return;
            }
            file.text().then((text) => resolve({ name: file.name, text }), reject);
        });
        document.body.append(input);
        input.click();
    });
}
