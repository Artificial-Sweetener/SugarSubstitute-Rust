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
/** Build an isolated UXP package from authored source without installing or launching Photoshop. */
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, mkdtempSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { packageFiles, packagePath, record, text, validatePackage } from './manifest.js';

/** Compile, stage and verify one package; an entirely absent runtime is a valid preparation state. */
export async function buildPhotoshop(
  root: string,
  checkOnly: boolean,
  development = false,
): Promise<boolean> {
  const sourceRoot = path.join(root, 'integrations/photoshop');
  const config = record(
    JSON.parse(readFileSync(path.join(sourceRoot, 'build.json'), 'utf8')),
    'build',
  );
  const sourceEntry = packagePath(sourceRoot, text(config.entry, 'entry'));
  const sourceManifest = path.join(sourceRoot, 'manifest.json');
  if (config.output !== 'build/photoshop' || config.target !== 'es2022')
    throw new Error('Review changes to the UXP output root or JavaScript target in build tooling.');
  if (!existsSync(sourceEntry) && !existsSync(sourceManifest)) return false;
  if (!existsSync(sourceEntry) || !existsSync(sourceManifest))
    throw new Error('Plugin scaffolding requires both manifest.json and the TypeScript entry.');
  const stagingRoot = path.join(root, '.generated/photoshop');
  mkdirSync(stagingRoot, { recursive: true });
  const staging = mkdtempSync(path.join(stagingRoot, 'package-'));
  const output = path.join(root, config.output);
  if (
    !staging.startsWith(`${path.resolve(root)}${path.sep}.generated${path.sep}`) ||
    !output.startsWith(`${path.resolve(root)}${path.sep}build${path.sep}`)
  )
    throw new Error('Plugin build output must stay within its declared repository roots.');
  try {
    const staticRoot = packagePath(sourceRoot, text(config.static, 'static'));
    for (const source of packageFiles(staticRoot)) {
      if (!/\.(?:html|css|svg|png|jpe?g|gif|webp|json|txt|woff2?|ttf|otf)$/.test(source))
        throw new Error(`Unsupported static plugin resource: ${path.basename(source)}`);
      const target = packagePath(staging, path.relative(staticRoot, source).replaceAll('\\', '/'));
      mkdirSync(path.dirname(target), { recursive: true });
      copyFileSync(source, target);
    }
    copyFileSync(sourceManifest, path.join(staging, 'manifest.json'));
    await build({
      entryPoints: [sourceEntry],
      outfile: path.join(staging, 'main.js'),
      bundle: true,
      format: 'cjs',
      platform: 'neutral',
      target: 'es2022',
      external: ['uxp', 'photoshop'],
      tsconfig: path.join(root, 'tsconfig.photoshop.json'),
      sourcemap: development ? 'linked' : false,
      legalComments: 'inline',
      metafile: true,
      logLevel: 'silent',
      plugins: [
        {
          name: 'uxp-runtime-modules',
          setup(context) {
            context.onResolve({ filter: /^[^./]/ }, (args) => {
              if (args.kind === 'entry-point') return undefined;
              if (args.path === 'uxp' || args.path === 'photoshop')
                return { path: args.path, external: true };
              throw new Error(`Native UXP dependencies require explicit review: ${args.path}`);
            });
          },
        },
      ],
    });
    const license = path.join(root, 'LICENSE');
    if (existsSync(license)) copyFileSync(license, path.join(staging, 'LICENSE'));
    validatePackage(staging, !development);
    if (!checkOnly) {
      // Both deletion targets are fixed descendants validated above; source and deployment trees are disjoint.
      rmSync(output, { recursive: true, force: true });
      for (const source of packageFiles(staging)) {
        const target = path.join(output, path.relative(staging, source));
        mkdirSync(path.dirname(target), { recursive: true });
        copyFileSync(source, target);
      }
    }
    return true;
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}

/** Build or validate a package without changing Adobe application state. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const built = await buildPhotoshop(root, process.argv.includes('--check'));
  console.log(
    built
      ? 'Photoshop package validated.'
      : 'Photoshop runtime is not scaffolded; repository preparation is valid.',
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
