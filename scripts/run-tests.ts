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
import { spawn } from 'node:child_process';
import type { SpawnOptions } from 'node:child_process';
import type { Readable, Writable } from 'node:stream';

const SUPPRESSED_WARNINGS = [
  'ExperimentalWarning: VM Modules is an experimental feature',
  'Use `node --trace-warnings',
];

function shouldSuppress(line: string): boolean {
  return SUPPRESSED_WARNINGS.some((token) => line.includes(token));
}

function forwardStream(stream: Readable, writer: Writable): void {
  let buffer = '';
  stream.on('data', (chunk: Buffer | string) => {
    buffer += chunk.toString();
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() ?? '';
    for (const line of lines) {
      if (!shouldSuppress(line)) {
        writer.write(`${line}\n`);
      }
    }
  });
  stream.on('end', () => {
    if (buffer && !shouldSuppress(buffer)) {
      writer.write(buffer);
    }
  });
}

function runProcess(
  file: string,
  args: readonly string[] = [],
  options: SpawnOptions = {},
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(file, args, options);
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        const rendered = [file, ...args].filter(Boolean).join(' ');
        reject(new Error(`${rendered} exited with ${code}`));
      }
    });
  });
}

function runJestWithFilter(): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn('node', ['--experimental-vm-modules', 'node_modules/jest/bin/jest.js'], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    forwardStream(child.stdout, process.stdout);
    forwardStream(child.stderr, process.stderr);
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`jest exited with ${code}`));
      }
    });
  });
}

async function main(): Promise<void> {
  const results = await Promise.allSettled([
    runJestWithFilter(),
    runProcess('python', ['-m', 'pytest'], { stdio: 'inherit' }),
  ]);
  const failures = results.filter(
    (result): result is PromiseRejectedResult => result.status === 'rejected',
  );
  if (failures.length) {
    const errors = failures
      .map((result) => (result.reason instanceof Error ? result.reason.message : 'test failed'))
      .join('\n');
    throw new Error(errors);
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
