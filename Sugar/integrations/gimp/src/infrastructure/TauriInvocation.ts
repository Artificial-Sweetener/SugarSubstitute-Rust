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
/** Isolate Tauri's dynamic public invocation API behind one typed adapter boundary. */

/** Narrow bridge objects without accepting arrays or inheriting unvalidated host values. */
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('The Tauri companion bridge is unavailable.');
  return value as Record<string, unknown>;
}

/** Preserve the native receiver while keeping every response unknown until its consumer validates it. */
export async function invokeTauri(
  host: object,
  command: string,
  args?: Readonly<Record<string, unknown>>,
): Promise<unknown> {
  const bridge = record(Reflect.get(host, '__TAURI__'));
  const core = record(bridge.core);
  if (typeof core.invoke !== 'function')
    throw new Error('The Tauri companion bridge is unavailable.');
  const result: unknown = await core.invoke.call(core, command, args);
  return result;
}
