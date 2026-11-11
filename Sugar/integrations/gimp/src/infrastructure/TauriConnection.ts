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
/** Translate Tauri's public connection command into validated, framework-independent values. */
import type { ConnectionState } from '../application/CompanionConnection.js';
import { invokeTauri } from './TauriInvocation.js';

/** Narrow dynamic bridge and response objects before crossing the application boundary. */
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('The companion returned an invalid value.');
  return value as Record<string, unknown>;
}

/** Invoke only the local shell's fixed connection command. */
export async function connectCompanion(host: object, session: string): Promise<ConnectionState> {
  const result = await invokeTauri(host, 'connect_comfy', { session });
  const state = record(result);
  if (
    (state.phase !== 'disconnected' &&
      state.phase !== 'connected' &&
      state.phase !== 'connecting') ||
    typeof state.address !== 'string' ||
    state.address.length > 2048 ||
    typeof state.message !== 'string' ||
    state.message.length > 512
  )
    throw new Error('The companion returned invalid connection state.');
  return { phase: state.phase, address: state.address, message: state.message };
}
