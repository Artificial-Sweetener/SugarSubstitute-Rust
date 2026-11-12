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
/** Prove the host description is one closed schema whose names stay open to later additions. */
import { readFileSync } from 'node:fs';
import { hostBindings } from '../../../scripts/integrations/generate-host-contract.js';
import { isHostDescriptionMessage } from '../../../integrations/shared/HostValidation.generated.js';
import {
  DeclaredHost,
  hostDescriptionMessage,
} from '../../../integrations/shared/HostDescription.js';
import {
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
} from '../../../integrations/contracts/HostDescription.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
}

const vectors = JSON.parse(
  readFileSync('tests/editor_integrations/support/host_wire_vectors.json', 'utf8'),
) as Vector[];
const session = '00000000-0000-4000-8000-000000000001';

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await hostBindings(
    readFileSync('integrations/contracts/host.schema.json', 'utf8'),
  );
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/HostProtocol.generated.rs',
    'integrations/contracts/HostProtocol.generated.ts',
    'integrations/shared/HostValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('shared host vectors preserve the closed version-one envelope', () => {
  for (const vector of vectors)
    expect([vector.label, isHostDescriptionMessage(vector.value)]).toEqual([
      vector.label,
      vector.valid,
    ]);
});

test('a host builds a schema-valid description that carries only its declarations', () => {
  const message = hostDescriptionMessage(session, {
    protocols: [HOST_PROTOCOLS.panel, HOST_PROTOCOLS.canvas],
    capabilities: [{ name: HOST_CAPABILITIES.hostStyles }],
  });
  expect(isHostDescriptionMessage(message)).toBe(true);
  expect(message).toEqual({
    protocol: 'sugarcubes.host',
    version: 1,
    session,
    type: 'description',
    protocols: [
      { name: 'sugarcubes.panel', version: 3 },
      { name: 'sugarcubes.canvas', version: 3 },
    ],
    capabilities: [{ name: 'presentation.host-styles' }],
  });
});

test('a declaration serves exact protocol versions and ignores names it does not read', () => {
  const host = DeclaredHost.read({
    protocols: [
      { name: 'sugarcubes.canvas', version: 3 },
      { name: 'sugarcubes.future', version: 1 },
    ],
    capabilities: [{ name: 'future.capability' }, { name: HOST_CAPABILITIES.systemAccent }],
  });
  if (!host) throw new Error('A distinct description was refused.');
  expect(host.serves(HOST_PROTOCOLS.canvas)).toBe(true);
  expect(host.serves({ name: 'sugarcubes.canvas', version: 4 })).toBe(false);
  expect(host.serves(HOST_PROTOCOLS.panel)).toBe(false);
  expect(host.declares(HOST_CAPABILITIES.systemAccent)).toBe(true);
  expect(host.declares(HOST_CAPABILITIES.hostStyles)).toBe(false);
});

test('a description that repeats a protocol or capability is refused as ambiguous', () => {
  expect(
    DeclaredHost.read({
      protocols: [HOST_PROTOCOLS.canvas, { name: 'sugarcubes.canvas', version: 4 }],
      capabilities: [],
    }),
  ).toBeNull();
  expect(
    DeclaredHost.read({
      protocols: [],
      capabilities: [
        { name: HOST_CAPABILITIES.hostStyles },
        { name: HOST_CAPABILITIES.hostStyles },
      ],
    }),
  ).toBeNull();
});

test('declarations compare by membership and the built-in hosts declare what they name', () => {
  const left = DeclaredHost.read({
    protocols: [HOST_PROTOCOLS.panel, HOST_PROTOCOLS.canvas],
    capabilities: [
      { name: HOST_CAPABILITIES.hostStyles },
      { name: HOST_CAPABILITIES.systemAccent },
    ],
  });
  const right = DeclaredHost.read({
    protocols: [HOST_PROTOCOLS.canvas, HOST_PROTOCOLS.panel],
    capabilities: [
      { name: HOST_CAPABILITIES.systemAccent },
      { name: HOST_CAPABILITIES.hostStyles },
    ],
  });
  if (!left || !right) throw new Error('A distinct description was refused.');
  expect(left.same(right)).toBe(true);
  expect(left.same(DeclaredHost.undeclared)).toBe(false);
  for (const protocol of Object.values(HOST_PROTOCOLS))
    expect(DeclaredHost.standalone.serves(protocol)).toBe(false);
  expect(DeclaredHost.undeclared.serves(HOST_PROTOCOLS.panel)).toBe(true);
  expect(DeclaredHost.undeclared.serves(HOST_PROTOCOLS.canvas)).toBe(false);
  for (const capability of Object.values(HOST_CAPABILITIES)) {
    expect(DeclaredHost.standalone.declares(capability)).toBe(false);
    expect(DeclaredHost.undeclared.declares(capability)).toBe(false);
  }
});

test('Rust bindings bound every name as a lowercase token and every list by its maximum', async () => {
  const bindings = await hostBindings(
    readFileSync('integrations/contracts/host.schema.json', 'utf8'),
  );
  const rust = bindings['integrations/contracts/HostProtocol.generated.rs'] ?? '';
  expect(rust.match(/valid_lowercase_token\(&self\.name, 64\)/g)).toHaveLength(2);
  expect(rust).toContain('self.protocols.len() <= 32');
  expect(rust).toContain('self.capabilities.len() <= 64');
  expect(rust).toContain('(1..=65535).contains(&self.version)');
  expect(rust).toContain('self.protocol == "sugarcubes.host"');
});
