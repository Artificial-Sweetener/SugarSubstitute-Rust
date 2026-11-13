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
/** Compile the private native primitive boundary from one schema and shared transfer budgets. */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { format } from 'prettier';
import { contractBindings } from '../integrations/ContractBindings.js';
import { MAX_CANVAS_IMAGE_BYTES } from '../../integrations/shared/CanvasImagePolicy.js';
import { MAX_CANVAS_ENCODED_CHARACTERS } from '../../integrations/shared/CanvasImageEncoding.js';
import { MAX_CANVAS_RASTER_BYTES } from '../../integrations/shared/CanvasRasterEncoding.js';

/** Resolve external shared values once for both the native package and TypeScript guards. */
export async function nativeContractBindings(
  root: string,
): Promise<Readonly<Record<string, string>>> {
  const schema = JSON.parse(
    readFileSync(path.join(root, 'integrations/gimp/native.schema.json'), 'utf8'),
  ) as { $defs: Record<string, unknown> };
  const canvas = JSON.parse(
    readFileSync(path.join(root, 'integrations/contracts/canvas.schema.json'), 'utf8'),
  ) as { $defs: Record<string, { properties: Record<string, unknown> }> };
  const document = canvas.$defs.CanvasDocument;
  const raster = canvas.$defs.CanvasPixelRaster;
  const image = canvas.$defs.CanvasEncodedImage;
  const selection = canvas.$defs.CanvasSelectionBounds;
  if (!document || !raster || !image || !selection)
    throw new Error('The native boundary requires the shared canvas values.');
  Object.assign(schema.$defs, {
    CanvasWidth: document.properties.width,
    CanvasHeight: document.properties.height,
    CanvasSelectionBounds: selection,
    RasterWidth: raster.properties.width,
    RasterHeight: raster.properties.height,
    PngData: image.properties.data,
    RasterData: raster.properties.data,
  });
  const resolved = {
    ...schema,
    'x-generated-by': 'scripts/gimp/NativeContract.ts',
    'x-limits': {
      pngBytes: MAX_CANVAS_IMAGE_BYTES,
      rasterBytes: MAX_CANVAS_RASTER_BYTES,
      frameBytes: MAX_CANVAS_ENCODED_CHARACTERS + 4 * 1024 * 1024,
      pendingRequests: 8,
    },
  };
  return {
    ...(await contractBindings(JSON.stringify(resolved), {
      union: 'NativeMessage',
      schema: 'integrations/gimp/native.schema.json',
      generator: 'scripts/gimp/NativeContract.ts',
      types: 'integrations/gimp/src/contracts/NativeProtocol.generated.ts',
      validation: 'integrations/gimp/src/infrastructure/NativeValidation.generated.ts',
      importPath: '../contracts/NativeProtocol.generated.js',
      description: 'private GIMP native primitive messages',
    })),
    'integrations/gimp/native/native.schema.json': await format(JSON.stringify(resolved), {
      parser: 'json',
      printWidth: 100,
    }),
  };
}

/** Refuse stale generated protocol bindings before packaging or compiling a companion. */
export async function verifyNativeContract(root: string): Promise<void> {
  for (const [relative, content] of Object.entries(await nativeContractBindings(root)))
    if (readFileSync(path.join(root, relative), 'utf8').replaceAll('\r\n', '\n') !== content)
      throw new Error(`Regenerate the native contract: ${relative}`);
}

/** Generate explicitly; ordinary builds only verify generated inputs. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  if (process.argv.includes('--check')) return verifyNativeContract(root);
  for (const [relative, content] of Object.entries(await nativeContractBindings(root))) {
    const file = path.join(root, relative);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
