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
/** Read explicit Substitute settings through the browser file picker without modifying source files. */

/** Limit settings imports to a small JSON document and release the picker on selection or cancellation. */
export function pickSubstituteSettings(document: Document): Promise<unknown | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,application/json';
    input.hidden = true;
    document.body.append(input);
    /** Release the owned picker after every terminal selection path. */
    const finish = () => input.remove();
    input.addEventListener(
      'cancel',
      () => {
        finish();
        resolve(null);
      },
      { once: true },
    );
    input.addEventListener(
      'change',
      () => {
        const file = input.files?.[0];
        finish();
        if (!file) {
          resolve(null);
          return;
        }
        if (file.size > 1024 * 1024) {
          reject(new Error('The Substitute settings file must be smaller than 1 MiB.'));
          return;
        }
        void file
          .text()
          .then((text) => {
            const value: unknown = JSON.parse(text);
            resolve(value);
          })
          .catch(reject);
      },
      { once: true },
    );
    input.click();
  });
}
