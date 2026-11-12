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
/**
 * Gate protocol evolution: released protocol versions are frozen, capabilities are read only
 * when declared, and a changed behaviour takes a new capability name.
 */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@jest/globals';
import {
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
} from '../../../integrations/contracts/HostDescription.js';
import {
  declaredCapabilities,
  schemaDigest,
  schemaId,
  undeclaredHostReads,
  type ReleasedContracts,
} from '../support/ProtocolEvolution.js';

const contracts = 'integrations/contracts';
const released = JSON.parse(
  readFileSync(path.join(contracts, 'released.json'), 'utf8'),
) as ReleasedContracts;
const schemaFiles = readdirSync(contracts).filter((file) => file.endsWith('.schema.json'));
const capabilitySource = readFileSync(path.join(contracts, 'HostDescription.ts'), 'utf8');

/** Every authored page source, which is where declarations are read. */
function pageSources(folder = 'frontend'): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) return entry.name === 'node_modules' ? [] : pageSources(file);
    return /\.(ts|vue)$/.test(entry.name) && !entry.name.endsWith('.d.ts') ? [file] : [];
  });
}

test('every released protocol version keeps exactly the schema it was released with', () => {
  for (const file of schemaFiles) {
    const text = readFileSync(path.join(contracts, file), 'utf8');
    const entry = released.schemas.find((schema) => schema.id === schemaId(text));
    // A changed message set or envelope needs a new version beside the old one, or a capability.
    expect([file, entry?.file, entry?.sha256]).toEqual([file, file, schemaDigest(text)]);
  }
  for (const entry of released.schemas)
    expect([entry.id, schemaFiles.includes(entry.file)]).toEqual([entry.id, true]);
});

test('a schema digest changes with the schema meaning and ignores formatting', () => {
  const text = readFileSync(path.join(contracts, 'updates.schema.json'), 'utf8');
  const schema = JSON.parse(text) as { $defs: Record<string, { required?: string[] }> };
  expect(schemaDigest(JSON.stringify(schema))).toBe(schemaDigest(text));
  schema.$defs.UpdatesState?.required?.pop();
  expect(schemaDigest(JSON.stringify(schema))).not.toBe(schemaDigest(text));
});

test('released capability names keep their behaviour; a changed behaviour takes a new name', () => {
  const current = declaredCapabilities(capabilitySource);
  expect(current.map(({ name }) => name)).toEqual(Object.values(HOST_CAPABILITIES));
  for (const capability of released.capabilities)
    expect(current.find(({ name }) => name === capability.name)).toEqual(capability);
  for (const capability of current)
    expect(released.capabilities.map(({ name }) => name)).toContain(capability.name);
});

test('changing what a released capability means is detected', () => {
  const changed = capabilitySource.replace(
    'offered as a choice beside a custom one',
    'always used',
  );
  const meaning = (source: string): string | undefined =>
    declaredCapabilities(source).find(({ name }) => name === HOST_CAPABILITIES.systemAccent)
      ?.meaning;
  expect(meaning(changed)).not.toBe(meaning(capabilitySource));
});

const capabilityKeys = Object.keys(HOST_CAPABILITIES);
const protocolKeys = Object.keys(HOST_PROTOCOLS);
const capabilityNames = Object.values(HOST_CAPABILITIES);

test('the page reads a capability or protocol only through the host declaration', () => {
  const files = pageSources();
  expect(files.some((file) => file.endsWith('EmbeddedPanelHost.ts'))).toBe(true);
  const findings = files.flatMap((file) =>
    undeclaredHostReads(
      readFileSync(file, 'utf8'),
      capabilityKeys,
      protocolKeys,
      capabilityNames,
    ).map((finding) => `${file}: ${finding}`),
  );
  expect(findings).toEqual([]);
});

test.each([
  ['a table read outside declares', 'const accent = HOST_CAPABILITIES.systemAccent;'],
  ['a protocol read outside serves', 'if (HOST_PROTOCOLS.canvas.version > 2) start();'],
  ['a capability name written as text', "if (names.includes('output.host-surface')) take();"],
  ['an unknown capability', 'host.declares(HOST_CAPABILITIES.hostSurfaceV2);'],
  ['a declares call without the table', "host.declares('presentation.host-styles');"],
])('the declaration check rejects %s', (_label, source) => {
  expect(
    undeclaredHostReads(source, capabilityKeys, protocolKeys, capabilityNames).length,
  ).toBeGreaterThan(0);
});

test('the declaration check admits reads through the declaration', () => {
  expect(
    undeclaredHostReads(
      'if (host?.declares(HOST_CAPABILITIES.hostStyles) && host.serves(HOST_PROTOCOLS.canvas)) go();',
      capabilityKeys,
      protocolKeys,
      capabilityNames,
    ),
  ).toEqual([]);
});
