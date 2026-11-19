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
/** Prove the backend protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { backendBindings } from '../../../scripts/integrations/generate-backend-contract.js';
import {
  isBackendMessage,
  isBackendStateMessage,
} from '../../../integrations/shared/BackendValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

const SCHEMA = 'integrations/contracts/backend.schema.json';
const envelope = {
  protocol: 'sugarcubes.backend',
  version: 1,
  session: 'session-0123456789abcdef',
};

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await backendBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/BackendProtocol.generated.rs',
    'integrations/contracts/BackendProtocol.generated.ts',
    'integrations/shared/BackendValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the backend protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.backend).toEqual({ name: 'sugarcubes.backend', version: 1 });
});

test('the closed wire contract admits the state a host sends and the two requests of a page', () => {
  const valid = [
    { ...envelope, type: 'state', revision: 0, status: 'running' },
    { ...envelope, type: 'state', revision: 5, status: 'retry-ready' },
    { ...envelope, type: 'watch' },
    { ...envelope, type: 'retry', revision: 5 },
  ];
  const invalid = [
    { ...envelope, type: 'state', revision: 1, status: 'exploded' },
    { ...envelope, type: 'state', revision: -1, status: 'running' },
    { ...envelope, type: 'state', status: 'running' },
    { ...envelope, type: 'retry' },
    { ...envelope, type: 'retry', revision: 0 },
    { ...envelope, type: 'retry', revision: 5, force: true },
    { ...envelope, version: 2, type: 'watch' },
    { ...envelope, session: 'short', type: 'watch' },
  ];
  for (const value of valid) expect([value, isBackendMessage(value)]).toEqual([value, true]);
  for (const value of invalid) expect([value, isBackendMessage(value)]).toEqual([value, false]);
  expect(isBackendStateMessage(valid[0])).toBe(true);
  expect(isBackendStateMessage(valid[2])).toBe(false);
});
