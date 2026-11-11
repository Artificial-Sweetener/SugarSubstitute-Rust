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
/** Translate validated architecture inventory into editor boundary diagnostics. */
import { readFileSync } from 'node:fs';
import { auditIntegrations, type IntegrationBoundaries } from './audit.js';

/** Narrow JSON arrays before passing them to the compiler audit. */
function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}

/** Validate policy input without admitting dynamic host values into the audit. */
function boundaries(value: unknown): value is IntegrationBoundaries {
  return (
    !!value &&
    typeof value === 'object' &&
    'contracts' in value &&
    typeof value.contracts === 'string' &&
    'shared' in value &&
    typeof value.shared === 'string' &&
    'editors' in value &&
    Array.isArray(value.editors) &&
    value.editors.every(
      (editor: unknown) =>
        !!editor &&
        typeof editor === 'object' &&
        'name' in editor &&
        typeof editor.name === 'string' &&
        'root' in editor &&
        typeof editor.root === 'string' &&
        'infrastructure' in editor &&
        typeof editor.infrastructure === 'string' &&
        'modules' in editor &&
        strings(editor.modules),
    )
  );
}

const input: unknown = JSON.parse(readFileSync(0, 'utf8'));
if (
  !input ||
  typeof input !== 'object' ||
  !('files' in input) ||
  !strings(input.files) ||
  !('policy' in input) ||
  !boundaries(input.policy)
)
  throw new Error('Invalid editor architecture inventory.');
process.stdout.write(
  JSON.stringify(auditIntegrations(process.argv[2] ?? process.cwd(), input.files, input.policy)),
);
