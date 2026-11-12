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
/** Source-prepared complete native recipes share Core's actual reorder output with genuine command capture. */
import { requireValue } from '../../core/src/required.js';
import { Decoration, type DecorationSet } from '@codemirror/view';
import { prepareNativeReorderTargets, nativeReorderLineClass } from './reorder-native-targets.js';
import type { Range } from '@codemirror/state';
import {
  prepareDocumentValueReorders,
  serializeValueReorder,
  type PreparedValueReorder,
  type ReorderOutput,
  type SourceRange,
} from '../../core/src/index.js';
import type { NativeDocumentValue, PreparedNativeDocumentValues } from './document-values.js';
/** Native logical-document UTF-16 marks, lines and points remain invariant across transient gesture presentation. */
export interface NativeReorderRecipe {
  readonly key: number;
  readonly entry: NativeDocumentValue;
  readonly prepared: PreparedValueReorder;
  readonly decorations: DecorationSet;
  readonly output: ReorderOutput;
}
/** One current native-value publication owns its exact immutable per-scope compiled output. */
interface NativeRecipes {
  readonly byEntry: ReadonlyMap<NativeDocumentValue, NativeReorderRecipe>;
  readonly decorations: DecorationSet;
}
const prepared = new WeakMap<PreparedNativeDocumentValues, NativeRecipes>();
/** Cold source preparation alone compiles initial chip/carrier, line and empty-target ranges; hot gestures only borrow these actual outputs. */
export function prepareNativeReorderRecipes(values: PreparedNativeDocumentValues): void {
  if (prepared.has(values)) return;
  const batch = prepareDocumentValueReorders(values.scopes);
  if (!batch) throw new Error('native-reorder-scope-index-inconsistent');
  const byScope = new Map(batch.map((order) => [order.scope, order]));
  const recipes: NativeReorderRecipe[] = [],
    all: Range<Decoration>[] = [];
  for (const [key, entry] of values.entries.entries()) {
    if (!entry.scope || !entry.semanticMapping) continue;
    const order = byScope.get(entry.scope);
    if (!order || !order.chips.length) continue;
    const output = serializeValueReorder(order, order.initial);
    if (!output) throw new Error('native-reorder-initial-output-unavailable');
    const ranges: Range<Decoration>[] = [];
    /** The existing decoded mapping alone determines native logical endpoints, including CRLF and structured escapes. */
    const logical = (position: number): number => {
      const result = requireValue(entry.semanticMapping, 'reorder-native-recipe').decodedToLogical[
        position
      ];
      if (result == null) throw new Error('native-reorder-recipe-unmapped');
      return result;
    };
    /** Exact initial serialized carriers preserve native inline topology without parsing or another chip roster. */
    const mark = (range: SourceRange, id: number | null): void => {
      if (range.from >= range.to) return;
      const suffix = id === null ? 'gap' : 'chip';
      ranges.push(
        Decoration.mark({
          inclusive: true,
          nativeReorderValue: String(key),
          class: 'sp-reorder-' + suffix,
          attributes: {
            'data-reorder-native-value': String(key),
            ...(id === null ? {} : { 'data-reorder-chip': String(id) }),
            style:
              '--sp-native-outline:var(--sp-reorder-native-' +
              String(key) +
              '-outline,none);--sp-native-background:var(--sp-reorder-native-' +
              String(key) +
              '-fill,transparent);--sp-native-cursor:var(--sp-reorder-native-' +
              String(key) +
              '-cursor,inherit);--sp-native-hover-background:var(--sp-reorder-native-' +
              String(key) +
              '-hover-fill,transparent);--sp-native-visibility:var(--sp-reorder-native-' +
              String(key) +
              '-' +
              String(id) +
              '-visibility,visible);--sp-native-active-outline:var(--sp-reorder-native-' +
              String(key) +
              '-' +
              String(id) +
              '-outline);--sp-native-active-fill:var(--sp-reorder-native-' +
              String(key) +
              '-' +
              String(id) +
              '-fill)',
          },
        }).range(logical(range.from), logical(range.to)),
      );
    };
    let cursor = 0;
    for (const id of order.initial.order) {
      const range = requireValue(output.renderedRanges[id], 'reorder-native-recipe');
      mark({ from: cursor, to: range.from }, null);
      mark(range, id);
      cursor = range.to;
    }
    mark({ from: cursor, to: output.text.length }, null);
    for (const target of prepareNativeReorderTargets(order, output, entry.semanticMapping, key))
      ranges.push(target);
    const extent = entry.semanticMapping.logicalRange,
      last = values.document.lineAt(extent.to).number;
    for (let number = values.document.lineAt(extent.from).number; number <= last; number++)
      ranges.push(
        Decoration.line({
          class: nativeReorderLineClass(String(key)),
          nativeReorderValue: String(key),
        }).range(values.document.line(number).from),
      );
    const decorations = Decoration.set(ranges, true);
    for (const range of ranges) all.push(range);
    recipes.push(Object.freeze({ key, entry, prepared: order, output, decorations }));
  }
  prepared.set(
    values,
    Object.freeze({
      byEntry: new Map(recipes.map((recipe) => [recipe.entry, recipe])),
      decorations: Decoration.set(all, true),
    }),
  );
}
/** Borrow one exact current entry; absence never triggers preparation on native input. */
export function nativeReorderRecipe(
  values: PreparedNativeDocumentValues,
  entry: NativeDocumentValue,
): NativeReorderRecipe | null {
  return prepared.get(values)?.byEntry.get(entry) ?? null;
}
/** One last canonical exclusion is process memoization keyed by the exact source publication and actual excluded entry, never another state/roster. */
const exclusions = new WeakMap<
  PreparedNativeDocumentValues,
  {
    readonly entry: NativeDocumentValue;
    readonly decorations: DecorationSet;
  }
>();
/** Native source painting retains the complete stable recipe, excluding only the actual whole canonical replacement through supported bounded filtering. */
export function nativeReorderRecipeDecorations(
  values: PreparedNativeDocumentValues | undefined,
  excludedEntry: NativeDocumentValue | null = null,
): DecorationSet {
  const source = values && prepared.get(values);
  if (!source || !values) return Decoration.none;
  if (!excludedEntry) return source.decorations;
  const recipe = source.byEntry.get(excludedEntry),
    range = excludedEntry.semanticMapping?.logicalRange;
  if (!recipe || !range) throw new Error('native-reorder-exclusion-foreign-entry');
  const previous = exclusions.get(values);
  if (previous?.entry === excludedEntry) return previous.decorations;
  const decorations = source.decorations.update({
    filterFrom: values.document.lineAt(range.from).from,
    filterTo: range.to,
    filter: (_from, _to, value) => value.spec.nativeReorderValue !== String(recipe.key),
  });
  exclusions.set(values, { entry: excludedEntry, decorations });
  return decorations;
}
/** Only native markers belonging to the genuine selected value participate in input/copy/admission. */
export function nativeReorderElement(node: Element, key: number): boolean {
  return node.getAttribute('data-reorder-native-value') === String(key);
}
