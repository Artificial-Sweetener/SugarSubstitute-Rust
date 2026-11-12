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
/** Generate update status bindings through the common contract compiler. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { contractBindings } from './ContractBindings.js';
import { rustContractBindings } from './RustContractBindings.js';

/** Compile TypeScript types, guards and Rust decoders from the one updates schema. */
export async function updatesBindings(
  schemaText: string,
): Promise<Readonly<Record<string, string>>> {
  const typescript = await contractBindings(schemaText, {
    union: 'UpdatesMessage',
    schema: 'updates.schema.json',
    generator: 'scripts/integrations/generate-updates-contract.ts',
    types: 'integrations/contracts/UpdatesProtocol.generated.ts',
    validation: 'integrations/shared/UpdatesValidation.generated.ts',
    importPath: '../contracts/UpdatesProtocol.generated.js',
    description: 'every version-one update status message',
  });
  const schema = JSON.parse(schemaText) as { $defs: Record<string, unknown> };
  return {
    ...typescript,
    'integrations/contracts/UpdatesProtocol.generated.rs': rustContractBindings(
      schema.$defs,
      schemaText,
      {
        schema: 'updates.schema.json',
        generator: 'scripts/integrations/generate-updates-contract.ts',
        prefix: 'Updates',
      },
    ),
  };
}

/** Generate or check bindings without modifying application or host state. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const sources = await updatesBindings(
    readFileSync(path.join(root, 'integrations/contracts/updates.schema.json'), 'utf8'),
  );
  for (const [relative, source] of Object.entries(sources)) {
    const file = path.join(root, relative);
    if (process.argv.includes('--check')) {
      if (readFileSync(file, 'utf8').replaceAll('\r\n', '\n') !== source)
        throw new Error(`Regenerate ${relative}.`);
    } else writeFileSync(file, source);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
