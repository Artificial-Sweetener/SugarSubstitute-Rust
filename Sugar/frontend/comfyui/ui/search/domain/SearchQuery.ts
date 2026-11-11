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
/** Parse Substitute's three finder contexts without exposing browser or graph dependencies. */
export type SearchMode = 'text' | 'field' | 'node';

export interface SearchQuery {
  readonly mode: SearchMode;
  readonly raw: string;
  readonly normalized: string;
  readonly node: string;
  readonly text: string;
  readonly tokens: readonly string[];
}

export interface SearchInput {
  readonly mode: SearchMode;
  readonly text: string;
  readonly pending: boolean;
}

/** Consume a completed context command, keeping incomplete commands out of matching. */
export function parseSearchInput(mode: SearchMode, text: string): SearchInput {
  const command = /^\s*@(text|field|node)\s+/i.exec(text);
  if (command) {
    const context = command[1]?.toLowerCase();
    if (context === 'text' || context === 'field' || context === 'node')
      return { mode: context, text: text.slice(command[0].length).trimStart(), pending: false };
  }
  return { mode, text, pending: text.trimStart().startsWith('@') };
}

/** Split a Node query at its first quoted text portion, accepting an unfinished closing quote. */
export function buildSearchQuery(mode: SearchMode, raw: string): SearchQuery {
  const normalized = raw.trim().toLowerCase();
  const quote = raw.indexOf('"');
  const node = mode === 'node' ? (quote < 0 ? raw : raw.slice(0, quote)).trim().toLowerCase() : '';
  const after = quote < 0 ? '' : raw.slice(quote + 1);
  const close = after.indexOf('"');
  const text =
    mode === 'text'
      ? normalized
      : mode === 'node'
        ? (close < 0 ? after : after.slice(0, close)).trim().toLowerCase()
        : '';
  return {
    mode,
    raw,
    normalized,
    node,
    text,
    tokens: (mode === 'node' ? node : normalized).split(/\s+/).filter(Boolean),
  };
}
