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
/**
 * Python's settings search (`settings_search.py`): the query is case-folded and split into words,
 * and a row matches when every query word starts at least one word of its page's title and
 * subtitle, its section's title and subtitle, its own title and description, or its keywords.
 * Results keep page, section and row order.
 */
import type { SettingsPage, SettingsRow, SettingsSection } from './SettingsModel.js';

/** One search result: a row and where it lives. */
export interface SettingsSearchResult {
  readonly page: SettingsPage;
  readonly section: SettingsSection;
  readonly row: SettingsRow;
}

/** Split text into searchable words: runs of word characters, braces and `#`. */
export function searchWords(text: string): string[] {
  return text
    .toLocaleLowerCase()
    .split(/[^\p{L}\p{N}_{}#]+/u)
    .filter(Boolean);
}

/** Every row of the searchable pages that the query matches, in page, section and row order. */
export function searchSettings(
  pages: readonly SettingsPage[],
  query: string,
): SettingsSearchResult[] {
  const wanted = searchWords(query);
  if (!wanted.length) return [];
  const results: SettingsSearchResult[] = [];
  for (const page of pages) {
    if (page.searchable === false) continue;
    for (const section of page.sections)
      for (const row of section.rows) {
        if (row.body || row.visible?.read() === false) continue;
        const words = searchWords(
          [
            page.title,
            page.subtitle,
            section.title,
            section.subtitle ?? '',
            row.title,
            row.description ?? '',
            ...(row.keywords ?? []),
          ].join(' '),
        );
        if (wanted.every((word) => words.some((candidate) => candidate.startsWith(word))))
          results.push({ page, section, row });
      }
  }
  return results;
}
