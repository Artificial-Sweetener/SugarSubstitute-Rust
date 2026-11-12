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
/** Prove the update status protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { updatesBindings } from '../../../scripts/integrations/generate-updates-contract.js';
import {
  isUpdatesClientMessage,
  isUpdatesHostMessage,
  isUpdatesMessage,
} from '../../../integrations/shared/UpdatesValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
}

const vectors = JSON.parse(
  readFileSync('tests/editor_integrations/support/updates_wire_vectors.json', 'utf8'),
) as Vector[];

/** Read the message type a vector carries, when it carries one. */
function typeOf(value: unknown): unknown {
  return value && typeof value === 'object' ? (value as { type?: unknown }).type : undefined;
}

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await updatesBindings(
    readFileSync('integrations/contracts/updates.schema.json', 'utf8'),
  );
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/UpdatesProtocol.generated.rs',
    'integrations/contracts/UpdatesProtocol.generated.ts',
    'integrations/shared/UpdatesValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the update protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.updates).toEqual({ name: 'sugarcubes.updates', version: 1 });
});

test('shared update vectors preserve the closed version-one wire contract', () => {
  expect(vectors.some((vector) => vector.valid)).toBe(true);
  for (const vector of vectors)
    expect([vector.label, isUpdatesMessage(vector.value)]).toEqual([vector.label, vector.valid]);
});

test('direction guards admit only their own valid messages', () => {
  for (const { label, valid, value } of vectors) {
    const host = ['state', 'accepted', 'rejected'].includes(String(typeOf(value)));
    expect([label, isUpdatesHostMessage(value)]).toEqual([label, valid && host]);
    expect([label, isUpdatesClientMessage(value)]).toEqual([label, valid && !host]);
  }
});

test('Rust bindings decode the status by its state tag and keep optional members present-only', async () => {
  const bindings = await updatesBindings(
    readFileSync('integrations/contracts/updates.schema.json', 'utf8'),
  );
  const rust = bindings['integrations/contracts/UpdatesProtocol.generated.rs'] ?? '';
  expect(rust).toContain(
    '#[serde(tag = "state", deny_unknown_fields, rename_all_fields = "camelCase")]',
  );
  expect(rust).toContain('pub(super) last_successful_check: Option<UpdatesCheck>,');
  expect(rust).toContain('pub(super) report: Option<UpdatesReport>,');
  expect(rust).toContain('valid_lowercase_token(&self.current_version, 64)');
  expect(rust).toContain('self.causes.iter().all(UpdatesCause::valid)');
});
