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
/** Prove the release notes protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { releaseNotesBindings } from '../../../scripts/integrations/generate-release-notes-contract.js';
import {
  isReleaseNotesClientMessage,
  isReleaseNotesHostMessage,
  isReleaseNotesMessage,
} from '../../../integrations/shared/ReleaseNotesValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
}

const SCHEMA = 'integrations/contracts/release-notes.schema.json';
const vectors = JSON.parse(
  readFileSync('integrations/contracts/release_notes_wire_vectors.json', 'utf8'),
) as Vector[];

/** Read the message type a vector carries, when it carries one. */
function typeOf(value: unknown): unknown {
  return value && typeof value === 'object' ? (value as { type?: unknown }).type : undefined;
}

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await releaseNotesBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/ReleaseNotesProtocol.generated.rs',
    'integrations/contracts/ReleaseNotesProtocol.generated.ts',
    'integrations/shared/ReleaseNotesValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the release notes protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.releaseNotes).toEqual({ name: 'sugarcubes.release-notes', version: 1 });
});

test('shared release notes vectors preserve the closed version-one wire contract', () => {
  expect(vectors.some((vector) => vector.valid)).toBe(true);
  expect(vectors.some((vector) => !vector.valid)).toBe(true);
  for (const vector of vectors)
    expect([vector.label, isReleaseNotesMessage(vector.value)]).toEqual([
      vector.label,
      vector.valid,
    ]);
});

test('direction guards admit only their own valid messages', () => {
  for (const { label, valid, value } of vectors) {
    const host = ['state', 'open', 'notes', 'failed'].includes(String(typeOf(value)));
    expect([label, isReleaseNotesHostMessage(value)]).toEqual([label, valid && host]);
    expect([label, isReleaseNotesClientMessage(value)]).toEqual([label, valid && !host]);
  }
});

test('notes text is bounded by UTF-16 units and a fetch always names its since version', () => {
  const schema = JSON.parse(readFileSync(SCHEMA, 'utf8')) as {
    $defs: Record<
      string,
      { properties: Record<string, { maxLength?: number }>; required: string[] }
    >;
  };
  expect(schema.$defs.ReleaseNotesRelease?.properties.notes?.maxLength).toBe(262144);
  expect(schema.$defs.ReleaseNotesLoadMessage?.required).toContain('since');
});
