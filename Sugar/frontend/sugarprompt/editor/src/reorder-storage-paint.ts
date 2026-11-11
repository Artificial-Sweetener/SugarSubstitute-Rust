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
/** Canonical stored-value recipes supply literal preview paint without another parser or coordinate authority. */
import { requireValue } from '../../core/src/required.js';
import {
  decodedRangeToSource,
  prepareWildcardCsvEncoding,
  type SourceRange,
  type PreparedWildcardCsvEncoding,
} from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { NativeReorderCapture } from './reorder-command.js';
import { atomicReorderOrigins } from './reorder-origin-fragments.js';
/** A publication's finite literal recipes retain the canonical mapping and exact affected storage edges. */
export interface ReorderStoragePaint {
  readonly replacement: SourceRange;
  readonly prefix: string;
  readonly suffix: string;
  literal(range: SourceRange): string;
}
const publications = new WeakMap<ReorderPreview, ReorderStoragePaint>();
const sources = new WeakMap<NativeReorderCapture, PreparedWildcardCsvEncoding>();
const captured = new WeakMap<NativeReorderCapture, Map<string, PreparedWildcardCsvEncoding>>();
/** One canonical captured-scope encoding is shared by every moved source interval, without duplicating overlapping byte/maps. */
export function prepareCapturedReorderStorage(
  capture: NativeReorderCapture,
): PreparedWildcardCsvEncoding | null {
  const previous = sources.get(capture);
  if (previous) return previous;
  const location = capture.target.scope.container.location;
  if (location.kind !== 'csv') return null;
  const prepared = prepareWildcardCsvEncoding(
    location.cell,
    capture.prepared.scope.document.sourceText,
  );
  sources.set(capture, prepared);
  return prepared;
}
/** Finite derived carriers and retained edge padding reuse their own canonical encoded output within one capture. */
function encoding(
  preview: ReorderPreview,
  key: string,
  text: () => string,
): PreparedWildcardCsvEncoding {
  let recipes = captured.get(preview.capture);
  if (!recipes) {
    recipes = new Map();
    captured.set(preview.capture, recipes);
  }
  const previous = recipes.get(key);
  if (previous) return previous;
  const location = preview.capture.target.scope.container.location;
  if (location.kind !== 'csv') throw new Error('reorder-storage-non-csv-recipe');
  const prepared = prepareWildcardCsvEncoding(location.cell, text());
  if (recipes.size === 64)
    recipes.delete(requireValue(recipes.keys().next().value, 'reorder-storage-paint'));
  recipes.set(key, prepared);
  return prepared;
}
/** Compose origin metadata; source fragments are encoded only on cold recipe misses, never scanned as a whole target. */
export function prepareReorderStoragePaint(preview: ReorderPreview): ReorderStoragePaint {
  const previous = publications.get(preview);
  if (previous) return previous;
  const { capture, output } = preview,
    container = capture.target.scope.container;
  let replacement = requireValue(
      capture.target.entry.semanticMapping,
      'reorder-storage-paint',
    ).logicalRange,
    prefix = '',
    suffix = '';
  const recipes =
    container.location.kind !== 'csv'
      ? []
      : atomicReorderOrigins(output).map((piece) => {
          if (piece.kind === 'composed') throw new Error('reorder-storage-non-atomic-origin');
          const prepared =
            piece.kind === 'derived'
              ? encoding(
                  preview,
                  `d:${output.text.slice(piece.outputRange.from, piece.outputRange.to)}`,
                  () => output.text.slice(piece.outputRange.from, piece.outputRange.to),
                )
              : requireValue(prepareCapturedReorderStorage(capture), 'reorder-storage-paint');
          return Object.freeze({
            range: piece.outputRange,
            prepared,
            offset: piece.kind === 'derived' ? 0 : piece.sourceRange.from,
          });
        });
  if (
    container.location.kind === 'csv' &&
    !container.location.cell.quoted &&
    recipes.some((recipe) => recipe.prepared.quoted)
  ) {
    replacement = capture.target.entry.logicalStorage;
    const original = requireValue(container.mapping, 'reorder-storage-paint'),
      leading = encoding(preview, 'padding:leading', () =>
        original.text.slice(0, container.valueRange.from),
      ),
      trailing = encoding(preview, 'padding:trailing', () =>
        original.text.slice(container.valueRange.to),
      );
    prefix =
      '"' + leading.text.slice(leading.mapping.sourceRange.from, leading.mapping.sourceRange.to);
    suffix =
      trailing.text.slice(trailing.mapping.sourceRange.from, trailing.mapping.sourceRange.to) +
      '"' +
      container.location.cell.postQuoteText;
  }
  /** Canonical decoration spans remain decoded UTF-16; ordinary literal nodes consume their encoded storage extent. */
  const literal = (range: SourceRange): string => {
    if (container.location.kind !== 'csv') return output.text.slice(range.from, range.to);
    let low = 0,
      high = recipes.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (requireValue(recipes[mid], 'reorder-storage-paint').range.to <= range.from) low = mid + 1;
      else high = mid;
    }
    const parts: string[] = [];
    for (
      let at = low;
      at < recipes.length &&
      requireValue(recipes[at], 'reorder-storage-paint').range.from < range.to;
      at++
    ) {
      const recipe = requireValue(recipes[at], 'reorder-storage-paint'),
        extent = decodedRangeToSource(recipe.prepared.mapping, {
          from: recipe.offset + Math.max(range.from, recipe.range.from) - recipe.range.from,
          to: recipe.offset + Math.min(range.to, recipe.range.to) - recipe.range.from,
        });
      if (!extent) throw new Error('reorder-storage-literal-boundary');
      parts.push(recipe.prepared.text.slice(extent.from, extent.to));
    }
    return parts.join('');
  };
  const prepared = Object.freeze({ replacement, prefix, suffix, literal });
  publications.set(preview, prepared);
  return prepared;
}
