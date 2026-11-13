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
/** Bind a distribution layout to its exact compiler-owned files and license resources. */
import { createHash } from 'node:crypto';

/** Record provenance after selecting a layout, excluding the inventory's own recursive hash. */
export function packageInventory(
  id: string,
  version: string,
  files: ReadonlyMap<string, Buffer>,
  development: boolean,
): Buffer {
  return Buffer.from(
    JSON.stringify(
      {
        id,
        version,
        platform: 'windows-x86_64',
        development,
        files: [...files].map(([name, data]) => ({
          name,
          sha256: createHash('sha256').update(data).digest('hex'),
        })),
      },
      null,
      2,
    ) + '\n',
  );
}
