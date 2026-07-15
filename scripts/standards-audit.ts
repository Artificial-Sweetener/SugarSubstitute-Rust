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
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

function repoRoot(rootDir: string = process.cwd()): string {
  return path.resolve(rootDir);
}

function walkFiles(rootDir: string, predicate: (filePath: string) => boolean): string[] {
  const results: string[] = [];
  const stack = [rootDir];
  while (stack.length) {
    const current = stack.pop();
    if (current === undefined) {
      continue;
    }
    for (const entry of readdirSync(current)) {
      const fullPath = path.join(current, entry);
      const stat = statSync(fullPath);
      if (stat.isDirectory()) {
        stack.push(fullPath);
        continue;
      }
      if (predicate(fullPath)) {
        results.push(fullPath);
      }
    }
  }
  return results.sort();
}

function previousNonEmptyLine(lines: readonly string[], startIdx: number): number {
  let idx = startIdx - 1;
  while (idx >= 0 && !lines[idx]?.trim()) {
    idx -= 1;
  }
  return idx;
}

function hasLeadingJsdoc(content: string): boolean {
  const contentAfterLicense = content.replace(
    /^(?:\/\/ {4}SugarCubes - composable workflow units for ComfyUI\r?\n\/\/ {4}Copyright \(C\) .+?\r?\n\/\/\r?\n\/\/ {4}This program is free software:[\s\S]+?\/\/ {4}along with this program\. {2}If not, see <https:\/\/www\.gnu\.org\/licenses\/>\.\r?\n)/,
    '',
  );
  return contentAfterLicense.trimStart().startsWith('/**');
}

function hasExportJsdoc(lines: readonly string[], exportIdx: number): boolean {
  const previousIdx = previousNonEmptyLine(lines, exportIdx);
  return previousIdx >= 0 && Boolean(lines[previousIdx]?.trim().endsWith('*/'));
}

/** Audit authored frontend and tooling code against repository standards. */
export function auditStandards(rootDir: string = process.cwd()): string[] {
  const root = repoRoot(rootDir);
  const failures: string[] = [];
  const frontendFiles = walkFiles(path.join(root, 'frontend'), (fullPath) =>
    fullPath.endsWith('.ts'),
  );
  for (const fullPath of frontendFiles) {
    const relativePath = path.relative(root, fullPath).replaceAll('\\', '/');
    const content = readFileSync(fullPath, 'utf8');
    if (!hasLeadingJsdoc(content)) {
      failures.push(`${relativePath} is missing a top-level JSDoc module comment`);
    }
    const lines = content.split(/\r?\n/);
    lines.forEach((line, idx) => {
      if (
        /^\s*export\s+(class|function|const)\s+[A-Za-z0-9_]+/.test(line) &&
        !hasExportJsdoc(lines, idx)
      ) {
        failures.push(`${relativePath}:${idx + 1} is missing JSDoc for an exported API`);
      }
    });
  }

  const authoredJavaScript = walkFiles(path.join(root, 'frontend'), (fullPath) =>
    fullPath.endsWith('.js'),
  );
  for (const fullPath of authoredJavaScript) {
    failures.push(
      `${path.relative(root, fullPath).replaceAll('\\', '/')} is JavaScript in the TypeScript source tree`,
    );
  }

  const runtimeTypeScript = walkFiles(path.join(root, 'web'), (fullPath) =>
    fullPath.endsWith('.ts'),
  );
  for (const fullPath of runtimeTypeScript) {
    failures.push(
      `${path.relative(root, fullPath).replaceAll('\\', '/')} is TypeScript in the generated runtime tree`,
    );
  }

  const scriptFiles = walkFiles(path.join(root, 'scripts'), (fullPath) => fullPath.endsWith('.ts'));
  for (const fullPath of scriptFiles) {
    const relativePath = path.relative(root, fullPath).replaceAll('\\', '/');
    if (relativePath === 'scripts/standards-audit.ts') {
      continue;
    }
    const content = readFileSync(fullPath, 'utf8');
    if (content.includes('shell: true')) {
      failures.push(`${relativePath} uses shell: true`);
    }
  }

  return failures;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const failures = auditStandards();
  if (failures.length) {
    console.error('Web/tooling standards audit failed:');
    for (const failure of failures) {
      console.error(` - ${failure}`);
    }
    process.exit(1);
  }
  console.log('Web/tooling standards audit passed.');
}
