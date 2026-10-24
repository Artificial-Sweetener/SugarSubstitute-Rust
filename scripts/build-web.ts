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
import { copyFileSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';
import { browserDependencyResolution } from './BrowserBuildEnvironment.js';
import { walkArtifactFiles, synchronizeWebArtifacts } from './WebArtifactSynchronization.js';
import { browserSharedModulePlugin, publishBrowserBundle } from './BrowserSharedModules.js';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const runtimeRoot = path.join(repositoryRoot, 'web');
const buildRoot = process.env.SUGARCUBES_WEB_BUILD_ROOT ?? path.join(repositoryRoot, '.generated');
if (!path.isAbsolute(buildRoot)) throw new Error('SUGARCUBES_WEB_BUILD_ROOT must be absolute.');
const generatedRoot = path.join(buildRoot, 'web');
if (path.resolve(generatedRoot) === runtimeRoot)
  throw new Error('Web staging must be separate from the runtime deployment tree.');
const staticRoot = path.join(repositoryRoot, 'frontend', 'static');

/** Compile authored TypeScript into the isolated staging directory. */
function compileTypeScript(): Promise<void> {
  const compilerPath = path.join(repositoryRoot, 'node_modules', 'typescript', 'bin', 'tsc');
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [compilerPath, '--project', 'tsconfig.web.json', '--outDir', generatedRoot],
      {
        cwd: repositoryRoot,
        stdio: 'inherit',
      },
    );
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
  for (const source of walkArtifactFiles(staticRoot)) {
    const output = path.join(generatedRoot, path.relative(staticRoot, source));
    mkdirSync(path.dirname(output), { recursive: true });
    copyFileSync(source, output);
  }
  const noticesRoot = path.join(repositoryRoot, 'third_party');
  for (const source of walkArtifactFiles(noticesRoot)) {
    const output = path.join(generatedRoot, 'third_party', path.relative(noticesRoot, source));
    mkdirSync(path.dirname(output), { recursive: true });
    copyFileSync(source, output);
  }
}

/** Bundle Vue surfaces while preserving canonical shared-module identities across relocated chunks. */
async function bundleVueSurface(
  relativeRoots: string | readonly string[],
  sharedModules: readonly string[],
): Promise<void> {
  const sharedOwners = [
    ...sharedModules,
    'surface/appearance/ThemeService.js',
    'surface/appearance/ControlFactory.js',
    'surface/appearance/VisualStyleBoundary.js',
    'surface/appearance/FluentThemeRuntime.js',
    'surface/appearance/SpectrumThemeRuntime.js',
  ];
  const sourceRoot = path.join(repositoryRoot, 'frontend/comfyui/ui');
  const roots = typeof relativeRoots === 'string' ? [relativeRoots] : relativeRoots;
  const entries = roots
    .flatMap((root) => walkArtifactFiles(path.join(sourceRoot, root)))
    .filter((filePath) => filePath.endsWith('.ts'));
  if (entries.length > 0) {
    const outputRoot = path.join(
      generatedRoot,
      'comfyui/ui',
      typeof relativeRoots === 'string' ? relativeRoots : '',
    );
    const sharedRoot = path.join(generatedRoot, 'comfyui/ui');
    const result = await build({
      ...browserDependencyResolution,
      entryPoints: entries,
      outbase:
        typeof relativeRoots === 'string' ? path.join(sourceRoot, relativeRoots) : sourceRoot,
      outdir: outputRoot,
      bundle: true,
      splitting: true,
      loader: { '.txt': 'text' },
      external: ['/scripts/*'],
      plugins: [browserSharedModulePlugin(sourceRoot, sharedRoot, sharedOwners)],
      write: false,
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
    publishBrowserBundle(result.outputFiles, outputRoot, sharedRoot, sharedOwners);
  }
}

/** Build the complete browser deployment tree or verify its committed state. */
async function main(): Promise<void> {
  const checkOnly = process.argv.includes('--check');
  rmSync(generatedRoot, { force: true, recursive: true });
  await compileTypeScript();
  for (const source of walkArtifactFiles(path.join(generatedRoot, 'frontend'))) {
    const output = path.join(
      generatedRoot,
      path.relative(path.join(generatedRoot, 'frontend'), source),
    );
    mkdirSync(path.dirname(output), { recursive: true });
    copyFileSync(source, output);
  }
  rmSync(path.join(generatedRoot, 'frontend'), { recursive: true, force: true });
  rmSync(path.join(generatedRoot, 'integrations'), { recursive: true, force: true });
  for (const runtime of ['PanelRuntime', 'CanvasRuntime'])
    await build({
      entryPoints: [path.join(repositoryRoot, `frontend/comfyui/ui/core/embedded/${runtime}.ts`)],
      outfile: path.join(generatedRoot, `comfyui/ui/core/embedded/${runtime}.js`),
      bundle: true,
      format: 'esm',
      platform: 'browser',
      target: 'es2022',
      legalComments: 'inline',
    });
  await build({
    ...browserDependencyResolution,
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
    ...browserDependencyResolution,
    entryPoints: [path.join(repositoryRoot, 'frontend/comfyui/ui/menus/MenuControlRuntime.ts')],
    outfile: path.join(generatedRoot, 'comfyui/ui/menus/MenuControlRuntime.js'),
    external: [
      '../surface/appearance/SpectrumThemeRuntime.js',
      '../surface/appearance/VisualStyleBoundary.js',
    ],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    legalComments: 'inline',
  });
  await build({
    ...browserDependencyResolution,
    entryPoints: [
      path.join(repositoryRoot, 'frontend/comfyui/ui/surface/appearance/SpectrumThemeRuntime.ts'),
    ],
    outfile: path.join(generatedRoot, 'comfyui/ui/surface/appearance/SpectrumThemeRuntime.js'),
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    legalComments: 'inline',
  });
  await bundleVueSurface('model_library/vue', [
    'model_library/ModelVersionDialog.js',
    'model_library/ModelDetectorDialog.js',
    'menus/ContextMenu.js',
    'menus/FloatingSurface.js',
  ]);
  await bundleVueSurface('prompt_editor/runtime', [
    'menus/ContextMenu.js',
    'menus/FloatingSurface.js',
    'surface/appearance/ThemeService.js',
    'surface/appearance/ControlFactory.js',
  ]);
  await build({
    ...browserDependencyResolution,
    entryPoints: [
      path.join(repositoryRoot, 'frontend/comfyui/ui/prompt_editor/PromptLoraCatalog.ts'),
    ],
    outfile: path.join(generatedRoot, 'comfyui/ui/prompt_editor/PromptLoraCatalog.js'),
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    legalComments: 'inline',
  });
  rmSync(path.join(generatedRoot, 'sugarprompt'), { force: true, recursive: true });
  await bundleVueSurface(
    ['surface/stack/vue', 'picker/vue', 'output_canvas/presentation/vue'],
    ['menus/ContextMenu.js', 'menus/FloatingSurface.js'],
  );
  stageStaticAssets();
  await synchronizeWebArtifacts(generatedRoot, runtimeRoot, checkOnly);
  console.log(checkOnly ? 'Web artifacts are current.' : 'Web artifacts updated.');
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
