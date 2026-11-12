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
/** Prove unknown Tauri values are narrowed before entering the native session. */
import { jest } from '@jest/globals';
import { openNativeGimp } from '../../../integrations/gimp/src/infrastructure/TauriNativeGimp.js';

test('standalone diagnostics expose no native GIMP connection', async () => {
  const invoke = jest.fn(async () => null);
  await expect(openNativeGimp({ __TAURI__: { core: { invoke } } }, () => {})).resolves.toBeNull();
  expect(invoke).toHaveBeenCalledWith('native_open', undefined);
});

test.each([undefined, true, 0, -1, '1', 1.5])(
  'rejects invalid renderer generation: %s',
  async (generation) => {
    await expect(
      openNativeGimp({ __TAURI__: { core: { invoke: async () => generation } } }, () => {}),
    ).rejects.toThrow('lifetime');
  },
);

test('validates replies and scopes every operation to its admitted renderer generation', async () => {
  const calls: unknown[][] = [];
  const invoke = async (command: unknown, args: unknown): Promise<unknown> => {
    calls.push([command, args]);
    if (command === 'native_open') return 3;
    if (command === 'native_submit') return 7;
    if (command === 'native_wait') return { kind: 'bogus' };
    return null;
  };
  const native = await openNativeGimp({ __TAURI__: { core: { invoke } } }, () => {});
  if (!native) throw new Error('Missing fixture native parent.');
  await expect(native.execute({ type: 'context' })).rejects.toThrow('response');
  expect(calls).toEqual([
    ['native_open', undefined],
    ['native_submit', { generation: 3, command: { type: 'context' } }],
    ['native_wait', { generation: 3, id: 7 }],
    ['native_cancel', { generation: 3, id: 7 }],
  ]);
  native.dispose();
  await Promise.resolve();
  expect(calls.at(-1)).toEqual(['native_close', { generation: 3 }]);
});
