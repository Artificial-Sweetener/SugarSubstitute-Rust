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
/** Resolve Substitute's display-only completion against the currently highlighted model. */
import type { ModelCatalogItem } from './ModelCatalog.js';

/** Keep the visible suggestion and its accepted search text in one completion decision. */
export interface ModelChoiceCompletion {
  readonly suffix: string;
  readonly text: string;
}

/** Complete the highlighted filename even before typing, then honor path and friendly-name searches. */
export function modelChoiceCompletion(
  item: ModelCatalogItem | undefined,
  query: string,
): ModelChoiceCompletion | null {
  if (!item) return null;
  const path = (item.relativePath.trim() || item.value.trim()).replaceAll('\\', '/');
  const stripped = path.replace(/\.(?:safetensors|ckpt|pt)$/i, '');
  const filename = stripped.split('/').at(-1) ?? stripped;
  const normalized = query.toLocaleLowerCase();
  const hasSeparator = /[\\/]/.test(query);
  if (hasSeparator && stripped.toLocaleLowerCase().startsWith(normalized.replaceAll('\\', '/'))) {
    const candidate =
      query.includes('\\') && !query.includes('/') ? stripped.replaceAll('/', '\\') : stripped;
    return completion(candidate.slice(query.length), query);
  }
  if (filename.toLocaleLowerCase().startsWith(normalized))
    return completion(filename.slice(query.length), query);
  if (!hasSeparator && query.trim().length > 0) {
    const start = filename.toLocaleLowerCase().indexOf(normalized);
    if (start > 0) return completion(filename.slice(start + query.length), query);
  }
  const friendly = [item.title.trim(), item.version.trim()].filter(Boolean).join(' - ');
  if (friendly.toLocaleLowerCase().startsWith(normalized))
    return completion(friendly.slice(query.length), query);
  return filename ? { suffix: ` → ${filename}`, text: filename } : null;
}

/** Accept ordinary completions by extending typed search, leaving exact matches unadorned. */
function completion(suffix: string, query: string): ModelChoiceCompletion | null {
  return suffix ? { suffix, text: query + suffix } : null;
}
