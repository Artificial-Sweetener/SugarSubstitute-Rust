/** @jest-environment node */
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
/** Prove the memory budget protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { memoryBindings } from '../../../scripts/integrations/generate-memory-contract.js';
import { rustContractBindings } from '../../../scripts/integrations/RustContractBindings.js';
import {
  isMemoryAllowanceMessage,
  isMemoryClientMessage,
  isMemoryMessage,
} from '../../../integrations/shared/MemoryValidation.generated.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
}

const vectors = JSON.parse(
  readFileSync('tests/editor_integrations/support/memory_wire_vectors.json', 'utf8'),
) as Vector[];

/** Read the message type a vector carries, when it carries one. */
function typeOf(value: unknown): unknown {
  return value && typeof value === 'object' ? (value as { type?: unknown }).type : undefined;
}

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await memoryBindings(
    readFileSync('integrations/contracts/memory.schema.json', 'utf8'),
  );
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/MemoryProtocol.generated.rs',
    'integrations/contracts/MemoryProtocol.generated.ts',
    'integrations/shared/MemoryValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('shared memory vectors preserve the closed version-one wire contract', () => {
  for (const vector of vectors)
    expect([vector.label, isMemoryMessage(vector.value)]).toEqual([vector.label, vector.valid]);
});

test('direction guards admit only their own valid messages', () => {
  for (const { label, valid, value } of vectors) {
    const allowance = typeOf(value) === 'allowance';
    expect([label, isMemoryAllowanceMessage(value)]).toEqual([label, valid && allowance]);
    expect([label, isMemoryClientMessage(value)]).toEqual([label, valid && !allowance]);
  }
});

test('Rust bindings decode the preference by its mode tag and bound consumer identities', async () => {
  const bindings = await memoryBindings(
    readFileSync('integrations/contracts/memory.schema.json', 'utf8'),
  );
  const rust = bindings['integrations/contracts/MemoryProtocol.generated.rs'] ?? '';
  expect(rust).toContain(
    '#[serde(tag = "mode", deny_unknown_fields, rename_all_fields = "camelCase")]',
  );
  expect(rust).toContain('Self::Share { percent } => (10..=75).contains(percent)');
  expect(rust).toContain('Self::Fixed { bytes } => (1..=9007199254740991).contains(bytes)');
  expect(rust).toContain('valid_lowercase_token(&self.id, 48)');
  expect(rust).toContain('self.consumers.len() <= 16');
  expect(rust).toContain('valid_panel_session(&self.session)');
  expect(rust).toContain('#[serde(rename_all = "kebab-case")]\npub(super) enum MemoryLevel');
});

/** Compile one string member through the Rust generator. */
function rustText(member: Record<string, unknown>): string {
  const definitions = {
    MemoryFixture: {
      type: 'object',
      properties: { id: member },
      required: ['id'],
      additionalProperties: false,
    },
  };
  return rustContractBindings(definitions, JSON.stringify(definitions), {
    schema: 'fixture.schema.json',
    generator: 'scripts/integrations/generate-fixture-contract.ts',
    prefix: 'Memory',
  });
}

test('Rust bindings admit a lowercase token only when its pattern and bound agree', () => {
  expect(rustText({ type: 'string', maxLength: 12, pattern: '^[a-z0-9.-]{1,12}$' })).toContain(
    'valid_lowercase_token(&self.id, 12)',
  );
  for (const member of [
    { type: 'string', maxLength: 40, pattern: '^[a-z0-9.-]{1,48}$' },
    { type: 'string', maxLength: 0, pattern: '^[a-z0-9.-]{1,0}$' },
    { type: 'string', maxLength: 257, pattern: '^[a-z0-9.-]{1,257}$' },
    { type: 'string', maxLength: 12, pattern: '^[a-z0-9_]{1,12}$' },
  ])
    expect(() => rustText(member)).toThrow('Rust contract strings require');
});
