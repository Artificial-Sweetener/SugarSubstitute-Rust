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
/** Prove the update report protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { updateReportBindings } from '../../../scripts/integrations/generate-update-report-contract.js';
import {
  isUpdateReportMessage,
  isUpdateReportStateMessage,
} from '../../../integrations/shared/UpdateReportValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

const SCHEMA = 'integrations/contracts/update-report.schema.json';
const envelope = {
  protocol: 'sugarcubes.update-report',
  version: 1,
  session: 'session-0123456789abcdef',
};

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await updateReportBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/UpdateReportProtocol.generated.rs',
    'integrations/contracts/UpdateReportProtocol.generated.ts',
    'integrations/shared/UpdateReportValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the update report protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.updateReport).toEqual({ name: 'sugarcubes.update-report', version: 1 });
});

test('the closed wire contract admits the state a host sends and the two requests of a page', () => {
  const valid = [
    { ...envelope, type: 'state', revision: 0, available: false },
    { ...envelope, type: 'state', revision: 5, available: true },
    { ...envelope, type: 'watch' },
    { ...envelope, type: 'open', revision: 5 },
    { ...envelope, type: 'open', revision: 0 },
  ];
  const invalid = [
    { ...envelope, type: 'state', revision: 1, available: 'yes' },
    { ...envelope, type: 'state', revision: -1, available: true },
    { ...envelope, type: 'state', available: true },
    { ...envelope, type: 'open' },
    { ...envelope, type: 'open', revision: 5, export: true },
    { ...envelope, version: 2, type: 'watch' },
    { ...envelope, session: 'short', type: 'watch' },
  ];
  for (const value of valid) expect([value, isUpdateReportMessage(value)]).toEqual([value, true]);
  for (const value of invalid)
    expect([value, isUpdateReportMessage(value)]).toEqual([value, false]);
  expect(isUpdateReportStateMessage(valid[0])).toBe(true);
  expect(isUpdateReportStateMessage(valid[2])).toBe(false);
});
