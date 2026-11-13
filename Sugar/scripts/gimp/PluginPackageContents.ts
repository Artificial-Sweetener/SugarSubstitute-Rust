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
/** Assemble the standard GIMP plug-in folder from the validated extension payload. */
import { GIMP_PLUGIN_DIRECTORY, type GimpPackageContents } from './PackageContents.js';
import { packageInventory } from './PackageInventory.js';

/** Preserve the runtime bytes and notices while qualifying provenance for the installed layout. */
export function pluginPackageContents(
  extension: GimpPackageContents,
  development: boolean,
): GimpPackageContents {
  const files = new Map<string, Buffer>();
  const prefix = `plug-ins/${GIMP_PLUGIN_DIRECTORY}/`;
  for (const [name, content] of extension.files) {
    if (name === 'package-inventory.json' || name === `${extension.id}.metainfo.xml`) continue;
    files.set(name.startsWith(prefix) ? name.slice(prefix.length) : name, content);
  }
  if (
    !files.has(`${GIMP_PLUGIN_DIRECTORY}.py`) ||
    !files.has('companion/sugarcubes-gimp-companion.exe')
  )
    throw new Error('The standard GIMP plug-in package is incomplete.');
  files.set(
    'package-inventory.json',
    packageInventory(extension.id, extension.version, files, development),
  );
  return { id: extension.id, version: extension.version, files };
}
