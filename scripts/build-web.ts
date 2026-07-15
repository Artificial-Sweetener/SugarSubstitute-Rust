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
  const modelsSource = path.join(staticRoot, 'models.txt');
  const modelsOutput = path.join(generatedRoot, 'models.txt');
  mkdirSync(path.dirname(modelsOutput), { recursive: true });
  copyFileSync(modelsSource, modelsOutput);
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

/** Build the complete browser deployment tree or verify its committed state. */
async function main(): Promise<void> {
  const checkOnly = process.argv.includes('--check');
  rmSync(generatedRoot, { force: true, recursive: true });
  await compileTypeScript();
  stageStaticAssets();
  synchronizeArtifacts(checkOnly);
  console.log(checkOnly ? 'Web artifacts are current.' : 'Web artifacts updated.');
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
