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
/** Adapt exact snapshot input into focused compiler-based browser ownership checks. */
import path from 'node:path';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';
import { auditInvalidation } from './invalidation-audit.js';
import { auditContextMenus, isMenuOwnership } from './context-menu-audit.js';

/** Validate one invocation before any ownership policy is applied. */
function main(): void {
  const root = path.resolve(process.argv[2] ?? process.cwd());
  const input: unknown = JSON.parse(readFileSync(0, 'utf8'));
  if (
    !input ||
    typeof input !== 'object' ||
    !('files' in input) ||
    !strings(input.files) ||
    !('owners' in input) ||
    !strings(input.owners) ||
    !('documentOwners' in input) ||
    !strings(input.documentOwners) ||
    !('menus' in input) ||
    !isMenuOwnership(input.menus)
  )
    throw new Error('Invalid browser UI policy inventory.');
  const program = ts.createProgram(
    input.files.map((file) => path.join(root, file)),
    {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.NodeNext,
      moduleResolution: ts.ModuleResolutionKind.NodeNext,
      strict: true,
      skipLibCheck: true,
    },
  );
  process.stdout.write(
    JSON.stringify([
      ...auditInvalidation(program, root, new Set(input.owners), new Set(input.documentOwners)),
      ...auditContextMenus(program, root, input.menus),
    ]),
  );
}

/** Narrow untrusted path inventories before constructing a compiler program. */
function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
