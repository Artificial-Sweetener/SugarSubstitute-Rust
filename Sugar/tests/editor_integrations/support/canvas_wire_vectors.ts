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
/** Read language-neutral wire vectors shared with native hosts and expand their mutations. */
import { readFileSync } from 'node:fs';

/** Describe one invalid message as a single edit of a named valid message. */
interface CanvasWireMutation {
  readonly name: string;
  readonly base: string;
  readonly path: readonly (string | number)[];
  readonly value?: unknown;
  readonly delete?: boolean;
}

/** Hold complete valid messages and the single-member mutations each host must refuse. */
export interface CanvasWireVectors {
  readonly valid: Readonly<Record<string, unknown>>;
  readonly invalid: readonly CanvasWireMutation[];
}

/** Load one vector file from the editor integration support directory. */
export function readCanvasWireVectors(file: string): CanvasWireVectors {
  return JSON.parse(
    readFileSync(`tests/editor_integrations/support/${file}`, 'utf8'),
  ) as CanvasWireVectors;
}

/** Return a deep copy of one valid message so tests can alter it freely. */
export function canvasWireMessage(vectors: CanvasWireVectors, name: string): unknown {
  if (!(name in vectors.valid)) throw new Error(`Unknown wire vector ${name}.`);
  return structuredClone(vectors.valid[name]);
}

/** Apply each mutation to a copy of its base message. */
export function invalidCanvasWireMessages(
  vectors: CanvasWireVectors,
): readonly { readonly name: string; readonly message: unknown }[] {
  return vectors.invalid.map((mutation) => {
    const message = canvasWireMessage(vectors, mutation.base);
    const last = mutation.path.at(-1);
    let cursor: unknown = message;
    for (const segment of mutation.path.slice(0, -1))
      cursor = cursor && typeof cursor === 'object' ? Reflect.get(cursor, segment) : undefined;
    if (!cursor || typeof cursor !== 'object' || last === undefined)
      throw new Error(`Invalid wire mutation ${mutation.name}.`);
    if (mutation.delete) Reflect.deleteProperty(cursor, last);
    else Reflect.set(cursor, last, mutation.value);
    return { name: mutation.name, message };
  });
}
