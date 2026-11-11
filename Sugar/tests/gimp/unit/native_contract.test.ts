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
/** Prove generated companion guards admit bounded fractional native captures without coercion. */
import { readFileSync } from 'node:fs';
import { nativeContractBindings } from '../../../scripts/gimp/NativeContract.js';
import {
  isNativeCommand,
  isNativeMessage,
} from '../../../integrations/gimp/src/infrastructure/NativeValidation.generated.js';

test('native protocol outputs remain compiled from the shared budget owners and private schema', async () => {
  for (const [file, source] of Object.entries(await nativeContractBindings(process.cwd())))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test.each([1, 0.5, 0.0001])('admits the exact numeric capture scale: %s', (scale) => {
  expect(
    isNativeCommand({ type: 'snapshot.selection', token: 'fixture', width: 4, height: 3, scale }),
  ).toBe(true);
});

test.each([true, '0.5', NaN, Infinity, -Infinity, -0.1, 1.001])(
  'refuses invalid numeric capture scale: %s',
  (scale) => {
    expect(
      isNativeCommand({ type: 'snapshot.selection', token: 'fixture', width: 4, height: 3, scale }),
    ).toBe(false);
  },
);

test('keeps the private envelope closed and request identifiers unambiguous', () => {
  const request = { type: 'request', id: 7, command: { type: 'context' } };
  expect(isNativeMessage(request)).toBe(true);
  expect(isNativeMessage({ ...request, extra: true })).toBe(false);
  expect(isNativeMessage({ ...request, id: true })).toBe(false);
  expect(isNativeMessage({ ...request, command: { type: 'context', extra: true } })).toBe(false);
  expect(isNativeCommand({ type: 'snapshot.release', token: 'fixture\n' })).toBe(false);
});
