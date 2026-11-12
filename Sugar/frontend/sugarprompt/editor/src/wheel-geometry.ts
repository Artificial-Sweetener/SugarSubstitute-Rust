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
/** Measured weighted-token hit geometry uses prepared semantics and supported native DOM-position APIs. */
import type { EditorView } from '@codemirror/view';
import type { SourceRange } from '../../core/src/index.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { weightSession } from './weight-session.js';
import { labelTarget, containsWeightPoint, type WeightRect } from './weight-control-geometry.js';
import type { WheelPoint } from './wheel-intent.js';
import {
  weightEditingAllowed,
  steppableLabelSelector,
  type WeightSubject,
} from './weight-target.js';

/** Cached visible rectangles and target bounds use viewport pixels and logical-document UTF-16 respectively. */
export interface WheelGeometry extends WeightSubject {
  readonly freshness: EditorFreshness;
  readonly rects: readonly WeightRect[];
}

/** Consume prepared rectangles without geometry reads in native wheel callbacks. */
export function wheelHit(geometry: WheelGeometry, point: WheelPoint): boolean {
  return geometry.rects.some((rect) => containsWeightPoint(rect, point.x, point.y));
}

/** Resolve an actually hit chip, numeric wildcard tag or innermost emphasis only during native measurement. */
export function measureWheelTarget(view: EditorView, point: WheelPoint): WheelGeometry | null {
  const position = view.posAtCoords(point);
  if (position === null) return null;
  const freshness = view.state.field(editorFreshness);
  const prepared = view.state.field(semanticStructures).native;
  if (!prepared || prepared.sourceIdentity !== freshness.source) return null;
  const element = document.elementFromPoint(point.x, point.y);
  if (!element || !view.contentDOM.contains(element)) return null;
  const chip = element.closest<HTMLElement>('.sp-lora-chip');
  const label =
    chip?.querySelector<HTMLElement>('.sp-lora-weight') ??
    element.closest<HTMLElement>(steppableLabelSelector);
  const labelSubject = label ? labelTarget(view, label) : null;
  const labelRange = labelSubject?.range;
  if (chip) {
    if (!labelSubject || labelSubject.kind !== 'lora') return null;
    const rect = chip.getBoundingClientRect();
    const geometry = {
      ...labelSubject,
      freshness,
      rects: [
        {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        },
      ],
    };
    return wheelHit(geometry, point) ? geometry : null;
  }
  if (label && labelSubject?.kind === 'wildcard') {
    const rect = label.getBoundingClientRect();
    const geometry: WheelGeometry = {
      ...labelSubject,
      freshness,
      rects: [
        {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        },
      ],
    };
    return wheelHit(geometry, point) ? geometry : null;
  }
  if (!weightEditingAllowed(view.state, 'emphasis')) return null;
  const session = view.state.field(weightSession);
  let range: SourceRange | null = null,
    outer: SourceRange | null = null,
    width = Infinity;
  if (
    session?.owner === 'wheel' &&
    session.neutral &&
    ((session.range.from <= position && position <= session.range.to) ||
      (labelRange?.from === session.range.from && labelRange.to === session.range.to))
  ) {
    range = session.range;
    outer = range;
  } else {
    prepared.pointerTargets.between(position, position, (from, to, target) => {
      if (target.kind !== 'emphasis' || to - from >= width) return;
      range = {
        from: from + target.contentFrom,
        to: from + target.contentTo,
      };
      outer = { from, to };
      width = to - from;
    });
  }
  if (!range || !outer) return null;
  const from = view.domAtPos(Math.max(outer.from, view.viewport.from)),
    to = view.domAtPos(Math.min(outer.to, view.viewport.to));
  const domRange = document.createRange();
  domRange.setStart(from.node, from.offset);
  domRange.setEnd(to.node, to.offset);
  const rects: WeightRect[] = Array.from(domRange.getClientRects(), (rect) => ({
    left: rect.left,
    top: rect.top,
    width: rect.width,
    height: rect.height,
  }));
  if (label && labelRange?.from === range.from && labelRange.to === range.to) {
    const rect = label.getBoundingClientRect();
    rects.push({
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    });
  }
  const geometry: WheelGeometry = { kind: 'emphasis', range, freshness, rects };
  return wheelHit(geometry, point) ? geometry : null;
}
