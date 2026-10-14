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
/** Own generated-file synchronization and exact nonmutating deployment verification. */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  lstatSync,
} from 'node:fs';
import path from 'node:path';

/** Inventory one compiler-owned tree without retaining file contents in memory. */
export function walkArtifactFiles(root: string): string[] {
  if (!existsSync(root)) return [];
  if (lstatSync(root).isSymbolicLink())
    throw new Error('Artifact roots must not be symbolic links.');
  const pending = [root];
  const files: string[] = [];
  while (pending.length > 0) {
    const current = pending.pop();
    if (current === undefined) continue;
    for (const entry of readdirSync(current)) {
      const fullPath = path.join(current, entry);
      const entryState = lstatSync(fullPath);
      if (entryState.isSymbolicLink())
        throw new Error('Artifact trees must not contain symbolic links.');
      if (entryState.isDirectory()) pending.push(fullPath);
      else files.push(fullPath);
    }
  }
  return files.sort();
}

/** Report stable relative paths independently of the workstation's root. */
function artifactPath(root: string, file: string): string {
  return path.relative(root, file).replaceAll('\\', '/');
}

/** Synchronize the isolated generated tree or reject any missing, stale or unexpected runtime file. */
export function synchronizeWebArtifacts(
  generatedRoot: string,
  runtimeRoot: string,
  checkOnly: boolean,
): void {
  if (!path.isAbsolute(generatedRoot) || !path.isAbsolute(runtimeRoot))
    throw new Error('Artifact roots must be absolute paths.');
  const separation = path.relative(generatedRoot, runtimeRoot);
  const reverse = path.relative(runtimeRoot, generatedRoot);
  if (!separation || contained(separation) || contained(reverse))
    throw new Error('Generated and runtime artifact roots must be separate trees.');
  const generatedPaths = walkArtifactFiles(generatedRoot);
  const runtimePaths = walkArtifactFiles(runtimeRoot);
  const expected = new Set(generatedPaths.map((file) => artifactPath(generatedRoot, file)));
  if (!checkOnly) {
    // Comfy can hold this directory open as its working directory on Windows.
    // Keep its identity and update only compiler-owned files beneath it.
    for (const runtimePath of runtimePaths) {
      if (!expected.has(artifactPath(runtimeRoot, runtimePath))) rmSync(runtimePath);
    }
    for (const generatedPath of generatedPaths) {
      const runtimePath = path.join(runtimeRoot, path.relative(generatedRoot, generatedPath));
      if (existsSync(runtimePath) && lstatSync(runtimePath).isDirectory()) {
        const relative = path.relative(runtimeRoot, runtimePath);
        if (!relative || !contained(relative))
          throw new Error('Artifact replacement escaped the runtime tree.');
        rmSync(runtimePath, { recursive: true });
      }
      if (existsSync(runtimePath) && readFileSync(generatedPath).equals(readFileSync(runtimePath)))
        continue;
      mkdirSync(path.dirname(runtimePath), { recursive: true });
      copyFileSync(generatedPath, runtimePath);
    }
    return;
  }
  const failures: string[] = [];
  for (const generatedPath of generatedPaths) {
    const rendered = artifactPath(generatedRoot, generatedPath);
    const runtimePath = path.join(runtimeRoot, path.relative(generatedRoot, generatedPath));
    if (!existsSync(runtimePath)) failures.push(`missing: ${rendered}`);
    else if (!readFileSync(generatedPath).equals(readFileSync(runtimePath)))
      failures.push(`stale: ${rendered}`);
  }
  for (const runtimePath of runtimePaths) {
    const rendered = artifactPath(runtimeRoot, runtimePath);
    if (!expected.has(rendered)) failures.push(`unexpected: ${rendered}`);
  }
  if (failures.length > 0)
    throw new Error(`Generated web artifacts are not synchronized:\n${failures.join('\n')}`);
}

/** Verify a relative replacement stays beneath its explicit compiler-owned root. */
function contained(relative: string): boolean {
  return !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('..' + path.sep);
}
