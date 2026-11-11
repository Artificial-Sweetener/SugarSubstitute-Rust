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
/** Generate the shared desktop-session values and closed shape guards from one schema. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { contractBindings } from './ContractBindings.js';
import { rustContractBindings } from './RustContractBindings.js';

/** Document public DTO fields and bound the tagged union footprint without changing JSON. */
function publicRust(source: string): string {
  return source
    .replaceAll('pub(super)', 'pub')
    .replaceAll('output: SessionOutput', 'output: Box<SessionOutput>')
    .replace(
      /^ {4}(\w+) \{ ([^{}\n]+) \},$/gm,
      (_match, variant: string, fields: string) =>
        `    ${variant} {\n${fields
          .split(', ')
          .map((field) => `        ${field},`)
          .join('\n')}\n    },`,
    )
    .replace(
      /^( {4}pub [a-z_]+:.*)$/gm,
      '    /// Schema-defined member; admit the complete value before use.\n$1',
    )
    .replace(
      /^( {8}[a-z_]+: [A-Za-z][^\n]*,?)$/gm,
      '        /// Correlation or payload fact; grants no authority.\n$1',
    )
    .replace(
      /^( {4}#\[serde\(rename = [^\n]+)$/gm,
      '    /// Schema-defined operation; lifecycle admission precedes effects.\n$1',
    );
}

/** Keep wire DTOs public; semantic admission belongs to the session contract validator. */
export async function sessionBindings(text: string): Promise<Readonly<Record<string, string>>> {
  const schema = JSON.parse(text) as { $defs: Record<string, unknown> };
  const generated = await contractBindings(text, {
    union: 'SessionValue',
    schema: 'session.schema.json',
    generator: 'scripts/integrations/generate-session-contract.ts',
    types: 'integrations/contracts/SessionProtocol.generated.ts',
    validation: 'integrations/shared/SessionValidation.generated.ts',
    importPath: '../contracts/SessionProtocol.generated.js',
    description: 'a version-one desktop checkpoint or session message',
    recordImport: './ContractRecord.js',
    validationModules: [
      {
        file: 'integrations/shared/SessionOutputValidation.generated.ts',
        definitions: [
          'SessionScope',
          'SessionSource',
          'SessionScene',
          'SessionFile',
          'SessionAsset',
          'SessionOutput',
          'SessionSubmission',
        ],
      },
      {
        file: 'integrations/shared/SessionViewValidation.generated.ts',
        definitions: [
          'SessionSelection',
          'SessionCamera',
          'SessionInspection',
          'SessionComparison',
          'SessionNavigation',
          'SessionView',
          'SessionGraphView',
        ],
      },
      {
        file: 'integrations/shared/SessionSnapshotValidation.generated.ts',
        definitions: [
          'SessionDocument',
          'SessionInput',
          'SessionEntry',
          'SessionRect',
          'SessionPlacement',
          'SessionCheckpoint',
        ],
      },
    ],
  });
  return {
    ...generated,
    'integrations/contracts/SessionProtocol.generated.rs': publicRust(
      rustContractBindings(schema.$defs, text, {
        schema: 'session.schema.json',
        generator: 'scripts/integrations/generate-session-contract.ts',
        prefix: 'Session',
      }),
    ),
  };
}

/** Regenerate deterministically or fail when the checked-in bindings have drifted. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const outputs = await sessionBindings(
    readFileSync(path.join(root, 'integrations/contracts/session.schema.json'), 'utf8'),
  );
  for (const [relative, source] of Object.entries(outputs)) {
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
