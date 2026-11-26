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
/** The messages the page and the spellchecker's worker exchange. */

/** Where the worker reads the module and the dictionary, and the person's own words. */
export interface SpellcheckLoad {
  readonly module: string;
  readonly aff: string;
  readonly dic: string;
  readonly words: readonly string[];
}

/** One request; `id` pairs it with its reply. */
export type SpellcheckRequest =
  | { readonly id: number; readonly kind: 'load'; readonly load: SpellcheckLoad }
  | { readonly id: number; readonly kind: 'check'; readonly words: readonly string[] }
  | { readonly id: number; readonly kind: 'suggest'; readonly word: string; readonly limit: number }
  | { readonly id: number; readonly kind: 'add'; readonly word: string };

/** One reply: the request's answer, or why it has none. */
export type SpellcheckReply =
  | { readonly id: number; readonly status: 'ready'; readonly value: unknown }
  | { readonly id: number; readonly status: 'failed'; readonly message: string };

/** Narrow a message from the worker to a reply, or null when it is not one. */
export function spellcheckReply(value: unknown): SpellcheckReply | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.id !== 'number') return null;
  if (record.status === 'ready') return { id: record.id, status: 'ready', value: record.value };
  if (record.status === 'failed' && typeof record.message === 'string')
    return { id: record.id, status: 'failed', message: record.message };
  return null;
}

/** Narrow a message to the worker to a request, or null when it is not one. */
export function spellcheckRequest(value: unknown): SpellcheckRequest | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  const id = record.id;
  if (typeof id !== 'number') return null;
  const strings = (items: unknown): items is string[] =>
    Array.isArray(items) && items.every((item) => typeof item === 'string');
  switch (record.kind) {
    case 'load': {
      const load = record.load as Record<string, unknown> | null | undefined;
      if (
        !load ||
        typeof load.module !== 'string' ||
        typeof load.aff !== 'string' ||
        typeof load.dic !== 'string' ||
        !strings(load.words)
      )
        return null;
      return {
        id,
        kind: 'load',
        load: { module: load.module, aff: load.aff, dic: load.dic, words: load.words },
      };
    }
    case 'check':
      return strings(record.words) ? { id, kind: 'check', words: record.words } : null;
    case 'suggest':
      return typeof record.word === 'string' && typeof record.limit === 'number'
        ? { id, kind: 'suggest', word: record.word, limit: record.limit }
        : null;
    case 'add':
      return typeof record.word === 'string' ? { id, kind: 'add', word: record.word } : null;
    default:
      return null;
  }
}
