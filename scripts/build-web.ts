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
/** Build and verify the committed ComfyUI JavaScript artifacts. */

import { spawn } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const generatedRoot = path.join(repositoryRoot, '.generated', 'web');
const runtimeRoot = path.join(repositoryRoot, 'web');
const staticRoot = path.join(repositoryRoot, 'frontend', 'static');

/** Return every file below a build-owned directory. */
function walkFiles(root: string): string[] {
  if (!existsSync(root)) {
    return [];
  }
  const pending = [root];
  const files: string[] = [];
  while (pending.length > 0) {
    const current = pending.pop();
    if (current === undefined) {
      continue;
    }
    for (const entry of readdirSync(current)) {
      const fullPath = path.join(current, entry);
      if (statSync(fullPath).isDirectory()) {
        pending.push(fullPath);
      } else {
        files.push(fullPath);
      }
    }
  }
  return files.sort();
}

/** Compile authored TypeScript into the isolated staging directory. */
function compileTypeScript(): Promise<void> {
  const compilerPath = path.join(repositoryRoot, 'node_modules', 'typescript', 'bin', 'tsc');
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [compilerPath, '--project', 'tsconfig.web.json'], {
      cwd: repositoryRoot,
      stdio: 'inherit',
    });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`TypeScript web build exited with ${code}.`));
      }
    });
  });
}

/** Stage authored static frontend assets beside compiled JavaScript. */
function stageStaticAssets(): void {
  for (const source of walkFiles(staticRoot)) {
    const output = path.join(generatedRoot, path.relative(staticRoot, source));
    mkdirSync(path.dirname(output), { recursive: true });
    copyFileSync(source, output);
  }
  const noticesRoot = path.join(repositoryRoot, 'third_party');
  for (const source of walkFiles(noticesRoot)) {
    const output = path.join(generatedRoot, 'third_party', path.relative(noticesRoot, source));
    mkdirSync(path.dirname(output), { recursive: true });
    copyFileSync(source, output);
  }
}

/** Render a stable repository-independent artifact path. */
function relativeArtifactPath(root: string, filePath: string): string {
  return path.relative(root, filePath).replaceAll('\\', '/');
}

/** Replace or verify the generated-only ComfyUI runtime tree. */
function synchronizeArtifacts(checkOnly: boolean): void {
  const generatedPaths = walkFiles(generatedRoot);
  if (!checkOnly) {
    rmSync(runtimeRoot, { force: true, recursive: true });
    for (const generatedPath of generatedPaths) {
      const relativePath = path.relative(generatedRoot, generatedPath);
      const runtimePath = path.join(runtimeRoot, relativePath);
      mkdirSync(path.dirname(runtimePath), { recursive: true });
      copyFileSync(generatedPath, runtimePath);
    }
    return;
  }

  const failures: string[] = [];
  const generatedRelativePaths = new Set(
    generatedPaths.map((filePath) => relativeArtifactPath(generatedRoot, filePath)),
  );
  for (const generatedPath of generatedPaths) {
    const relativePath = path.relative(generatedRoot, generatedPath);
    const renderedPath = relativeArtifactPath(generatedRoot, generatedPath);
    const runtimePath = path.join(runtimeRoot, relativePath);
    if (!existsSync(runtimePath)) {
      failures.push(`missing: ${renderedPath}`);
      continue;
    }
    const generated = readFileSync(generatedPath);
    const committed = readFileSync(runtimePath);
    if (!generated.equals(committed)) {
      failures.push(`stale: ${renderedPath}`);
    }
  }

  for (const runtimePath of walkFiles(runtimeRoot)) {
    const renderedPath = relativeArtifactPath(runtimeRoot, runtimePath);
    if (!generatedRelativePaths.has(renderedPath)) {
      failures.push(`unexpected: ${renderedPath}`);
    }
  }

  if (failures.length > 0) {
    throw new Error(`Generated web artifacts are not synchronized:\n${failures.join('\n')}`);
  }
}

/** Bundle each isolated Vue surface with shared chunks and explicit host-facing externals. */
async function bundleVueSurface(
  relativeRoots: string | readonly string[],
  external: string[],
): Promise<void> {
  const sourceRoot = path.join(repositoryRoot, 'frontend/comfyui/ui');
  const roots = typeof relativeRoots === 'string' ? [relativeRoots] : relativeRoots;
  const entries = roots
    .flatMap((root) => walkFiles(path.join(sourceRoot, root)))
    .filter((filePath) => filePath.endsWith('.ts'));
  if (entries.length > 0) {
    await build({
      entryPoints: entries,
      outbase:
        typeof relativeRoots === 'string' ? path.join(sourceRoot, relativeRoots) : sourceRoot,
      outdir: path.join(
        generatedRoot,
        'comfyui/ui',
        typeof relativeRoots === 'string' ? relativeRoots : '',
      ),
      bundle: true,
      splitting: true,
      external,
      format: 'esm',
      platform: 'browser',
      target: 'es2022',
      legalComments: 'inline',
      define: {
        __VUE_OPTIONS_API__: 'false',
        __VUE_PROD_DEVTOOLS__: 'false',
        __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: 'false',
        'process.env.NODE_ENV': '"production"',
      },
    });
  }
}

/** Build the complete browser deployment tree or verify its committed state. */
async function main(): Promise<void> {
  const checkOnly = process.argv.includes('--check');
  rmSync(generatedRoot, { force: true, recursive: true });
  await compileTypeScript();
  await build({
    entryPoints: [
      path.join(repositoryRoot, 'frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.ts'),
    ],
    outfile: path.join(generatedRoot, 'comfyui/ui/surface/appearance/FluentThemeRuntime.js'),
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    legalComments: 'inline',
  });
  await build({
    entryPoints: [path.join(repositoryRoot, 'frontend/comfyui/ui/menus/FluentMenuRuntime.ts')],
    outfile: path.join(generatedRoot, 'comfyui/ui/menus/FluentMenuRuntime.js'),
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    legalComments: 'inline',
  });
  await bundleVueSurface('model_library/vue', [
    '/scripts/*',
    '../ModelVersionDialog.js',
    '../ModelDetectorDialog.js',
    '../../menus/ContextMenu.js',
  ]);
  await bundleVueSurface(
    ['surface/stack/vue', 'picker/vue', 'output_canvas/presentation/vue'],
    ['/scripts/*', '../../../menus/ContextMenu.js'],
  );
  stageStaticAssets();
  synchronizeArtifacts(checkOnly);
  console.log(checkOnly ? 'Web artifacts are current.' : 'Web artifacts updated.');
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
