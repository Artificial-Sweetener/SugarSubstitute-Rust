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
/** The deployed editing chrome addresses every icon inside the served extension tree. */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { CANVAS_EDIT_ICON_NAMES } from '../../../frontend/comfyui/ui/canvas_editing/presentation/CanvasEditArtwork.js';
import { CANVAS_TOOL_ICONS } from '../../../frontend/comfyui/ui/canvas_editing/presentation/CanvasEditIcons.js';

const web = path.resolve('web');
const deployedArtwork = path.join(
  web,
  'comfyui/ui/canvas_editing/presentation/CanvasEditArtwork.js',
);

interface DeployedArtwork {
  readonly canvasEditIconUrl: (name: string) => string;
}

/** Load the compiled artwork module from its deployed location, as ComfyUI serves it. */
async function loadDeployedArtwork(): Promise<DeployedArtwork> {
  const loaded: unknown = await import(pathToFileURL(deployedArtwork).href);
  if (
    typeof loaded !== 'object' ||
    loaded === null ||
    !('canvasEditIconUrl' in loaded) ||
    typeof loaded.canvasEditIconUrl !== 'function'
  )
    throw new Error('The deployed artwork module exports no icon resolver.');
  const resolve = loaded.canvasEditIconUrl as (name: string) => unknown;
  return {
    canvasEditIconUrl: (name) => {
      const url = resolve(name);
      if (typeof url !== 'string') throw new Error(`No URL for ${name}.`);
      return url;
    },
  };
}

test('every tool icon is part of the artwork catalogue', () => {
  for (const icon of Object.values(CANVAS_TOOL_ICONS))
    expect(CANVAS_EDIT_ICON_NAMES).toContain(icon);
});

test('every deployed icon URL names an SVG inside the served extension tree', async () => {
  const artwork = await loadDeployedArtwork();
  const unresolved = CANVAS_EDIT_ICON_NAMES.filter((name) => {
    const file = fileURLToPath(artwork.canvasEditIconUrl(name));
    const within = path.relative(web, file);
    const escapes = within === '..' || within.startsWith(`..${path.sep}`);
    return escapes || !existsSync(file) || !statSync(file).isFile() || !file.endsWith('.svg');
  });
  expect(unresolved).toEqual([]);
});

test('bundled chunks reach the artwork through its own module rather than relocated URLs', () => {
  const chunks = readdirSync(path.join(web, 'comfyui/ui')).filter((name) =>
    /^chunk-.*\.js$/.test(name),
  );
  for (const chunk of chunks)
    expect(readFileSync(path.join(web, 'comfyui/ui', chunk), 'utf8')).not.toMatch(
      /canvas-editing\/\$\{/,
    );
});
