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
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import path from 'node:path';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const generatedRoot = path.join(repositoryRoot, '.generated', 'web');
const runtimeRoot = path.join(repositoryRoot, 'web');

function walkJavaScript(root: string): string[] {
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
      } else if (fullPath.endsWith('.js')) {
        files.push(fullPath);
      }
    }
  }
  return files.sort();
}

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

function synchronizeArtifacts(checkOnly: boolean): void {
  const stale: string[] = [];
  for (const generatedPath of walkJavaScript(generatedRoot)) {
    const relativePath = path.relative(generatedRoot, generatedPath);
    const runtimePath = path.join(runtimeRoot, relativePath);
    if (checkOnly) {
      const generated = readFileSync(generatedPath, 'utf8');
      const committed = readFileSync(runtimePath, 'utf8');
      if (generated !== committed) {
        stale.push(relativePath.replaceAll('\\', '/'));
      }
      continue;
    }
    mkdirSync(path.dirname(runtimePath), { recursive: true });
    copyFileSync(generatedPath, runtimePath);
  }
  if (stale.length > 0) {
    throw new Error(`Generated web artifacts are stale:\n${stale.join('\n')}`);
  }
}

async function main(): Promise<void> {
  const checkOnly = process.argv.includes('--check');
  rmSync(generatedRoot, { force: true, recursive: true });
  await compileTypeScript();
  synchronizeArtifacts(checkOnly);
  console.log(checkOnly ? 'Web artifacts are current.' : 'Web artifacts updated.');
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
