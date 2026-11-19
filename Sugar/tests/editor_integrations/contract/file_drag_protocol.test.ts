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
/** Prove the file drag protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { fileDragBindings } from '../../../scripts/integrations/generate-file-drag-contract.js';
import {
  isFileDragHostMessage,
  isFileDragMessage,
} from '../../../integrations/shared/FileDragValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

const SCHEMA = 'integrations/contracts/file-drag.schema.json';
const envelope = {
  protocol: 'sugarcubes.file-drag',
  version: 1,
  session: 'session-0123456789abcdef',
};

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await fileDragBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/FileDragProtocol.generated.rs',
    'integrations/contracts/FileDragProtocol.generated.ts',
    'integrations/shared/FileDragValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the file drag protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.fileDrag).toEqual({ name: 'sugarcubes.file-drag', version: 1 });
});

test('the closed wire contract admits file names and nothing else', () => {
  const file = { name: 'Portrait.json' };
  const valid = [
    { ...envelope, type: 'entered', files: [file] },
    { ...envelope, type: 'entered', files: Array.from({ length: 16 }, () => file) },
    { ...envelope, type: 'entered', files: [{ name: 'n'.repeat(255) }] },
    { ...envelope, type: 'left' },
  ];
  const invalid = [
    { ...envelope, type: 'entered', files: [] },
    { ...envelope, type: 'entered', files: Array.from({ length: 17 }, () => file) },
    { ...envelope, type: 'entered', files: [{ name: 'n'.repeat(256) }] },
    { ...envelope, type: 'entered', files: [{ name: 'a.json', path: 'C:/a.json' }] },
    { ...envelope, type: 'entered', files: ['a.json'] },
    { ...envelope, type: 'entered' },
    { ...envelope, type: 'left', files: [file] },
    { ...envelope, version: 2, type: 'left' },
    { ...envelope, session: 'short', type: 'left' },
    { ...envelope, type: 'watch' },
  ];
  for (const value of valid)
    expect([value, isFileDragMessage(value), isFileDragHostMessage(value)]).toEqual([
      value,
      true,
      true,
    ]);
  for (const value of invalid) expect([value, isFileDragMessage(value)]).toEqual([value, false]);
});
