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
/** Pointer indexes consume canonical decoded-to-logical tables outside native input. */
import { RangeSet, RangeValue, type Range } from '@codemirror/state';
import { wildcardBodyRange, type SourceRange } from '../../core/src/index.js';
import { segmentSelectionRange } from '../../core/src/prompt/selection.js';
import type { PreparedNativeDocumentValues } from './document-values.js';

/** A scope-qualified local token; content offsets are logical UTF-16 relative to its indexed range start. */
export class PointerTarget extends RangeValue {
  readonly kind: 'segment' | 'emphasis' | 'lora' | 'wildcard' | 'scene';
  readonly contentFrom: number;
  readonly contentTo: number;
  readonly scopeId: string;
  readonly ordinal: number;
  /** Capture immutable semantic identity and mapped content, never command authority. */
  constructor(
    kind: 'segment' | 'emphasis' | 'lora' | 'wildcard' | 'scene',
    contentFrom: number,
    contentTo: number,
    scopeId: string,
    ordinal: number,
  ) {
    super();
    this.kind = kind;
    this.contentFrom = contentFrom;
    this.contentTo = contentTo;
    this.scopeId = scopeId;
    this.ordinal = ordinal;
  }
}

/** Build one native RangeSet from actual scope documents and their already prepared coordinate tables. */
export function preparePointerTargets(
  values: PreparedNativeDocumentValues,
): RangeSet<PointerTarget> {
  const ranges: Range<PointerTarget>[] = [];
  for (const entry of values.entries) {
    const scope = entry.scope;
    const map = entry.semanticMapping;
    if (!scope || !map) continue;
    /** Reject an impossible parser endpoint without inventing scalar/quote/newline arithmetic. */
    const position = (decoded: number): number => {
      const logical = map.decodedToLogical[decoded];
      if (logical == null) throw new Error('pointer-endpoint-unmapped');
      return logical;
    };
    /** Input ranges are decoded scope-local UTF-16; the index stores logical CM UTF-16. */
    const add = (
      kind: PointerTarget['kind'],
      ordinal: number,
      outer: SourceRange,
      content: SourceRange = outer,
    ): void => {
      const from = position(outer.from),
        to = position(outer.to);
      ranges.push(
        new PointerTarget(
          kind,
          position(content.from) - from,
          position(content.to) - from,
          scope.id,
          ordinal,
        ).range(from, to),
      );
    };
    scope.document.segments.forEach((segment, ordinal) =>
      add('segment', ordinal, segmentSelectionRange(segment)),
    );
    scope.document.emphasisSpans.forEach((span, ordinal) =>
      add('emphasis', ordinal, span.outerRange, span.contentRange),
    );
    scope.document.loraSpans.forEach((span, ordinal) => add('lora', ordinal, span.outerRange));
    scope.document.wildcardSpans.forEach((span, ordinal) =>
      add('wildcard', ordinal, span.outerRange, wildcardBodyRange(span)),
    );
    scope.sceneContexts?.document.scenes.forEach(({ marker }, ordinal) =>
      add('scene', ordinal, marker.lineRange, marker.titleRange),
    );
  }
  return RangeSet.of(ranges, true);
}
