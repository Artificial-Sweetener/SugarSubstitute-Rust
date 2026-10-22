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
/** Canonical scope projection compiles literal separators and clipped rails outside native input and paint. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState, Range } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import type { SourceRange } from '../../core/src/index.js';
import { SourceToken } from './projection-widgets.js';
import { RegionWidget } from './region-widget.js';
import { preparedRegionPaint } from './region-paint.js';
import type { PreparedStructure } from './structure.js';
import type { RegionTarget } from './region-index.js';
import type { RegionHoverProjection } from './projection-contracts.js';
/** Derived ranges use logical CM UTF-16; literal ownership uses decoded scope-local UTF-16. */
interface RegionProjection {
  readonly decorations: readonly Range<Decoration>[];
  readonly tokens: readonly Range<SourceToken>[];
  readonly hovers: Readonly<Record<string, readonly RegionHoverProjection[]>>;
  /** Resolve structural precedence inside the actual canonical scope, never another value. */
  ownsDecodedRange(scopeId: string, range: SourceRange): boolean;
}
/** Retain actual region target identity and clip every rail to its admitted semantic value. */
export function prepareRegionProjection(
  state: EditorState,
  prepared: PreparedStructure,
): RegionProjection {
  const decorations: Range<Decoration>[] = [],
    tokens: Range<SourceToken>[] = [];
  const hovers: Record<string, readonly RegionHoverProjection[]> = Object.create(null) as Record<
    string,
    readonly RegionHoverProjection[]
  >;
  const targets = new Map<string, RegionTarget[]>();
  for (const target of prepared.regions) {
    const local = targets.get(target.scopeId) ?? [];
    local.push(target);
    targets.set(target.scopeId, local);
  }
  for (const entry of prepared.nativeValues.entries) {
    const scope = entry.scope,
      mapping = entry.semanticMapping;
    if (!scope || !mapping) continue;
    const structure = scope.document.regions,
      localTargets = targets.get(scope.id) ?? [],
      palette = preparedRegionPaint(scope).map((color) => {
        return `--sp-region-color:${color.normal};--sp-region-hover-color:${color.hover}`;
      });
    const hovered = structure.separators.map(() => [] as Range<Decoration>[]);
    /** Adapt canonical endpoints only; CSV storage and scalar gaps never become range bounds. */
    const position = (decoded: number): number => {
      const logical = mapping.decodedToLogical[decoded];
      if (logical == null) throw new Error('region-projection-endpoint-unmapped');
      return logical;
    };
    /** Prompt rails retain their full-line rendering; structured values use bounded source marks. */
    const rail = (
      from: number,
      to: number,
      attributes: Record<string, string>,
    ): Range<Decoration> | null =>
      prepared.kind === 'prompt'
        ? Decoration.line({ attributes }).range(from)
        : from < to
          ? Decoration.mark({ attributes }).range(from, to)
          : null;
    for (const [ordinal, separator] of structure.separators.entries()) {
      const target = localTargets[ordinal];
      if (!target || target.ordinal !== ordinal || target.separator !== separator)
        throw new Error('region-projection-target-mismatch');
      const from = target.from,
        to = target.to,
        token = new SourceToken();
      const highlight = rail(from, to, { class: 'sp-region-hover' });
      if (highlight) requireValue(hovered[ordinal], 'region-projection').push(highlight);
      const row = rail(from, to, {
        class: 'sp-region-row',
        style: requireValue(palette[ordinal], 'region-projection'),
      });
      if (row) decorations.push(row);
      decorations.push(
        Decoration.replace({ widget: new RegionWidget(token, target) }).range(from, to),
      );
      tokens.push(token.range(from, to));
    }
    for (const partition of structure.partitions) {
      if (partition.isGlobal) continue;
      const from = position(partition.sourceRange.from),
        to = position(partition.sourceRange.to);
      if (from === to && state.doc.lineAt(from).length !== 0) continue;
      for (let number = state.doc.lineAt(from).number; number <= state.doc.lines; number++) {
        const line = state.doc.line(number);
        if (line.from >= to && (from !== to || line.from !== from)) break;
        const clippedFrom = prepared.kind === 'prompt' ? line.from : Math.max(line.from, from),
          clippedTo = Math.min(line.to, to);
        const content = rail(clippedFrom, clippedTo, {
          class: 'sp-region-content',
          style: requireValue(palette[partition.index - 1], 'region-projection'),
        });
        const highlight = rail(clippedFrom, clippedTo, {
          class: 'sp-region-hover',
        });
        if (content) decorations.push(content);
        if (highlight)
          requireValue(hovered[partition.index - 1], 'region-projection').push(highlight);
        if (from === to) break;
      }
    }
    hovers[scope.id] = Object.freeze(
      hovered.map((ranges, ordinal) =>
        Object.freeze({
          target: requireValue(localTargets[ordinal], 'region-projection'),
          decorations: Decoration.set(ranges, true),
        }),
      ),
    );
  }
  return {
    decorations,
    tokens,
    hovers: Object.freeze(hovers),
    /** Region labels have one literal visual owner even when their spelling resembles rich syntax. */
    ownsDecodedRange(scopeId, range) {
      const separators = prepared.nativeValues.byId[scopeId]?.scope?.document.regions.separators;
      if (!separators) return false;
      let from = 0,
        to = separators.length;
      while (from < to) {
        const middle = (from + to) >>> 1;
        if (requireValue(separators[middle], 'region-projection').tokenRange.to <= range.from)
          from = middle + 1;
        else to = middle;
      }
      return (
        from < separators.length &&
        requireValue(separators[from], 'region-projection').tokenRange.from < range.to
      );
    },
  };
}
