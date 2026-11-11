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
/** Validate Comfy node definitions before exposing portable loader-field semantics. */
import { isRecord } from '../../types/common.js';
import {
  recognizeInputLoaders,
  type InputLoader,
  type InputAssetPorts,
  type InputKind,
} from '../domain/InputLoader.js';

/** Own the definition catalog supplied by Comfy's registration lifecycle. */
export class ComfyInputLoaders {
  readonly #loaders = new Map<string, readonly InputLoader[]>();
  readonly #ports = new Map<string, InputAssetPorts>();
  /** Decode all definitions once when Comfy publishes or refreshes its node catalog. */
  define(definitions: unknown): void {
    if (!isRecord(definitions)) return;
    for (const [classType, value] of Object.entries(definitions)) {
      if (!isRecord(value)) continue;
      const input = isRecord(value.input) ? value.input : {};
      const ports: Record<string, InputKind> = {};
      for (const section of ['required', 'optional']) {
        const entries = isRecord(input[section]) ? input[section] : {};
        for (const [name, field] of Object.entries(entries)) {
          const kind = assetKind(Array.isArray(field) ? field[0] : null);
          if (kind) ports[name] = kind;
        }
      }
      this.#ports.set(
        classType,
        Object.freeze({
          inputs: Object.freeze(ports),
          outputs: Object.freeze(Array.isArray(value.output) ? value.output.map(assetKind) : []),
        }),
      );
      const fields = ['required', 'optional'].flatMap((section) => {
        const entries = isRecord(input[section]) ? input[section] : {};
        return Object.entries(entries).map(([name, field]) => {
          const metadata = Array.isArray(field) && isRecord(field[1]) ? field[1] : {};
          return {
            name,
            upload: metadata.image_upload === true,
            folder: typeof metadata.image_folder === 'string' ? metadata.image_folder : null,
            ordered:
              metadata.allow_batch === true ||
              metadata.multiselect === true ||
              isRecord(metadata.multi_select),
          };
        });
      });
      const loaders = recognizeInputLoaders({
        classType,
        outputs: Array.isArray(value.output)
          ? value.output.filter((item): item is string => typeof item === 'string')
          : [],
        fields,
        channel: fields.some((field) => field.name === 'channel') ? 'channel' : null,
      });
      this.#loaders.set(classType, loaders);
    }
  }
  /** Enumerate independently configurable native upload fields in schema order. */
  all(classType: string): readonly InputLoader[] {
    return this.#loaders.get(classType) ?? [];
  }
  /** Copy immutable spatial declarations before queue preparation can await a definition refresh. */
  assetPorts(): ReadonlyMap<string, InputAssetPorts> {
    return new Map(this.#ports);
  }
  /** Resolve an exact field, or the sole unambiguous upload field. */
  read(classType: string, field?: string): InputLoader | null {
    const loaders = this.all(classType);
    return field === undefined
      ? loaders.length === 1
        ? (loaders[0] ?? null)
        : null
      : (loaders.find((loader) => loader.field === field) ?? null);
  }
}

/** Normalize spatial types while leaving unrelated Comfy sockets opaque. */
function assetKind(value: unknown): InputKind | null {
  return value === 'IMAGE' ? 'image' : value === 'MASK' ? 'mask' : null;
}
