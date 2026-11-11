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
/** Own rebuild coalescing and watcher cleanup independently of package compilation. */
import { watch } from 'node:fs';
import path from 'node:path';
import { buildPhotoshop } from './build.js';

/** Keep UDT output current across host and portable source changes. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const watching = true;
  const rebuild = async (): Promise<void> => {
    const built = await buildPhotoshop(root, false, watching);
    console.log(
      built
        ? 'Photoshop package validated.'
        : 'Photoshop runtime is not scaffolded; repository preparation is valid.',
    );
  };
  await rebuild();
  let pending: NodeJS.Timeout | undefined;
  let rebuilding = false;
  let dirty = false;
  const refresh = async (): Promise<void> => {
    if (rebuilding) {
      dirty = true;
      return;
    }
    rebuilding = true;
    do {
      dirty = false;
      try {
        await rebuild();
      } catch (error: unknown) {
        console.error(error);
      }
    } while (dirty);
    rebuilding = false;
  };
  const watchers = ['integrations/photoshop', 'integrations/contracts', 'integrations/shared'].map(
    (directory) =>
      watch(path.join(root, directory), { recursive: true }, () => {
        clearTimeout(pending);
        pending = setTimeout(() => {
          void refresh();
        }, 100);
      }),
  );
  process.once('SIGINT', () => {
    clearTimeout(pending);
    for (const watcher of watchers) watcher.close();
  });
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
