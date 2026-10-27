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
/** Bind the local shell's current native generation without granting SDK access to remote content. */
import { NativeGimpSession } from '../application/NativeGimpSession.js';
import { isNativeCommand, isNativeId, isNativeResult } from './NativeValidation.generated.js';
import { invokeTauri } from './TauriInvocation.js';

/** Open an optional native parent connection; standalone shell diagnostics retain their own lifetime. */
export async function openNativeGimp(
  host: object,
  report: (error: unknown) => void,
): Promise<NativeGimpSession | null> {
  const generation = await invokeTauri(host, 'native_open');
  if (generation === null) return null;
  if (!isNativeId(generation)) throw new Error('The native GIMP viewer lifetime is invalid.');
  return new NativeGimpSession(
    {
      submit: async (command) => {
        if (!isNativeCommand(command)) throw new Error('The native GIMP command is invalid.');
        const id = await invokeTauri(host, 'native_submit', { generation, command });
        if (!isNativeId(id)) throw new Error('The native GIMP request identity is invalid.');
        return id;
      },
      wait: async (id) => {
        const result = await invokeTauri(host, 'native_wait', { generation, id });
        if (!isNativeResult(result)) throw new Error('The native GIMP response is invalid.');
        return result;
      },
      cancel: async (id) => {
        await invokeTauri(host, 'native_cancel', { generation, id });
      },
      close: async () => {
        await invokeTauri(host, 'native_close', { generation });
      },
    },
    report,
  );
}
