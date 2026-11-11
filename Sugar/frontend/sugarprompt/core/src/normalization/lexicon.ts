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
/** Immutable exact-tag membership is prepared once and consumes the pinned Unicode identity rules. */
import { caseFold, strip, whitespace } from '../prompt/text.js';
import type { TagLexicon } from './contracts.js';

/** This fixed expression has no caller-owned state; String.replace resets its global cursor on every call. */
const identityWhitespace = new RegExp(whitespace.source + '+', 'gu');

/** Normalize dataset tag identity: underscores/spaces, Unicode whitespace and case folding. */
export function normalizeTagIdentity(text: string): string {
  return strip(caseFold(text.replaceAll('_', ' '))).replace(identityWhitespace, ' ');
}

/** Capture prepared tags without retaining caller-owned mutable data; empty tags never match. */
export function prepareTagLexicon(tags: readonly string[]): TagLexicon {
  const identities = new Set(tags.map(normalizeTagIdentity));
  identities.delete('');
  return Object.freeze({
    /** Inspect one normalized exact identity without traversing the catalog. */
    contains(text: string): boolean {
      return identities.has(normalizeTagIdentity(text));
    },
  });
}
