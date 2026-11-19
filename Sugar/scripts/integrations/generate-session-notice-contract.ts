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
/** Generate session notice bindings through the common contract compiler. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { contractBindings } from './ContractBindings.js';
import { rustContractBindings } from './RustContractBindings.js';

/** The released session notice protocol versions; each keeps its own schema and bindings. */
export const SESSION_NOTICE_VERSIONS = {
  1: { schema: 'session-notice.schema.json', stem: 'SessionNotice', words: 'version-one' },
  2: { schema: 'session-notice-v2.schema.json', stem: 'SessionNoticeV2', words: 'version-two' },
} as const;

/** Compile TypeScript types, guards and Rust decoders from one session notice schema version. */
export async function sessionNoticeBindings(
  schemaText: string,
  version: keyof typeof SESSION_NOTICE_VERSIONS = 1,
): Promise<Readonly<Record<string, string>>> {
  const { schema: schemaFile, stem, words } = SESSION_NOTICE_VERSIONS[version];
  const typescript = await contractBindings(schemaText, {
    union: 'SessionNoticeMessage',
    schema: schemaFile,
    generator: 'scripts/integrations/generate-session-notice-contract.ts',
    types: `integrations/contracts/${stem}Protocol.generated.ts`,
    validation: `integrations/shared/${stem}Validation.generated.ts`,
    importPath: `../contracts/${stem}Protocol.generated.js`,
    description: `every ${words} session notice message`,
  });
  const schema = JSON.parse(schemaText) as { $defs: Record<string, unknown> };
  return {
    ...typescript,
    [`integrations/contracts/${stem}Protocol.generated.rs`]: rustContractBindings(
      schema.$defs,
      schemaText,
      {
        schema: schemaFile,
        generator: 'scripts/integrations/generate-session-notice-contract.ts',
        prefix: 'SessionNotice',
      },
    ),
  };
}

/** Generate or check bindings without modifying application or host state. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  for (const version of [1, 2] as const) {
    const sources = await sessionNoticeBindings(
      readFileSync(
        path.join(root, 'integrations/contracts', SESSION_NOTICE_VERSIONS[version].schema),
        'utf8',
      ),
      version,
    );
    for (const [relative, source] of Object.entries(sources)) {
      const file = path.join(root, relative);
      if (process.argv.includes('--check')) {
        if (readFileSync(file, 'utf8').replaceAll('\r\n', '\n') !== source)
          throw new Error(`Regenerate ${relative}.`);
      } else writeFileSync(file, source);
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
