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
/** Freeze one loader's authored primitive settings for both previews and queued captures. */
import type { InputLoader, InputPreviewSettings } from './InputLoader.js';

/** Exclude the file field and reject dynamic/nonfinite values before crossing the native interpretation boundary. */
export function inputLoaderSettings(
  loader: InputLoader,
  values: Readonly<Record<string, unknown>>,
): InputPreviewSettings {
  const inputs: Record<string, string | number | boolean> = {};
  for (const [name, value] of Object.entries(values))
    if (
      name !== loader.field &&
      (typeof value === 'string' ||
        typeof value === 'boolean' ||
        (typeof value === 'number' && Number.isFinite(value)))
    )
      inputs[name] = value;
  return Object.freeze({
    kind: loader.kind,
    classType: loader.classType,
    field: loader.field,
    inputs: Object.freeze(inputs),
  });
}
