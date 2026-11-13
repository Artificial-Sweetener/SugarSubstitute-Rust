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
/** Prepared viewport geometry anchors two triangle controls without clipping the native editor. */
import type { EditorView } from '@codemirror/view';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { weightSession } from './weight-session.js';
import { weightEditingAllowed, type WeightSubject } from './weight-target.js';

/** Viewport pixel coordinates belong to native measurement, independently of source positions. */
export interface WeightRect {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}
/** A source-current logical-document UTF-16 target range and measured viewport-pixel arrow rectangles. */
export interface WeightControlGeometry extends WeightSubject {
  readonly freshness: EditorFreshness;
  readonly increase: WeightRect;
  readonly decrease: WeightRect;
  readonly activation: WeightRect;
  readonly centerOffset: number;
  readonly color: string;
}
/** Consume prepared rectangle data without reading DOM in pointer/timer callbacks. */
export function containsWeightPoint(rect: WeightRect, x: number, y: number): boolean {
  return (
    x >= rect.left && x <= rect.left + rect.width && y >= rect.top && y <= rect.top + rect.height
  );
}
/** Resolve a displayed number to logical-document UTF-16 range through the prepared index, including a transient neutral shell. */
export function labelTarget(view: EditorView, label: HTMLElement): WeightSubject | null {
  const kind = label.matches('.sp-lora-weight')
    ? 'lora'
    : label.matches(".sp-wildcard-tag[data-numeric='true']")
      ? 'wildcard'
      : 'emphasis';
  if (!weightEditingAllowed(view.state, kind)) return null;
  const position = view.posAtDOM(label, 0),
    session = view.state.field(weightSession);
  if (label.dataset.weightNeutral === 'true')
    return session?.neutral ? { kind: 'emphasis', range: session.range } : null;
  const prepared = view.state.field(semanticStructures).native;
  if (!prepared || prepared.sourceIdentity !== view.state.field(editorFreshness).source)
    return null;
  let result: WeightSubject | null = null,
    width = Infinity;
  prepared.pointerTargets.between(position, position, (from, to, value) => {
    if (
      value.kind === kind &&
      (kind === 'lora' ? from === position : from + value.contentTo === position) &&
      to - from < width
    ) {
      result = {
        kind,
        range:
          kind === 'wildcard'
            ? { from, to }
            : { from: from + value.contentFrom, to: from + value.contentTo },
      };
      width = to - from;
    }
  });
  return result;
}
/** Read current label geometry only in CodeMirror's measurement phase; preserve the initial horizontal offset across value changes. */
export function measureWeightControls(
  view: EditorView,
  label: HTMLElement,
  centerOffset: number | null,
): WeightControlGeometry | null {
  if (!label.isConnected || !view.contentDOM.contains(label)) return null;
  const target = labelTarget(view, label);
  if (!target) return null;
  const rect = label.getBoundingClientRect(),
    viewport = view.scrollDOM.getBoundingClientRect();
  if (
    rect.bottom < viewport.top ||
    rect.top > viewport.bottom ||
    rect.right < viewport.left ||
    rect.left > viewport.right
  )
    return null;
  const width = 13 * view.scaleX,
    height = 10 * view.scaleY;
  const offset = centerOffset ?? rect.width / 2;
  const left = Math.max(
    4,
    Math.min(rect.left + offset - width / 2, document.documentElement.clientWidth - width - 4),
  );
  const increase = {
    left,
    top: Math.max(4, rect.top - 0.5 * view.scaleY - height + 3 * view.scaleY),
    width,
    height,
  };
  const decrease = {
    left,
    top: Math.min(
      document.documentElement.clientHeight - height - 4,
      rect.bottom + 0.5 * view.scaleY - 3 * view.scaleY,
    ),
    width,
    height,
  };
  const top = Math.min(increase.top, rect.top),
    right = Math.max(rect.right, left + width),
    bottom = Math.max(rect.bottom, decrease.top + height);
  return {
    ...target,
    freshness: view.state.field(editorFreshness),
    increase,
    decrease,
    activation: {
      left: Math.min(left, rect.left),
      top,
      width: right - Math.min(left, rect.left),
      height: bottom - top,
    },
    centerOffset: offset,
    color: getComputedStyle(view.contentDOM).color,
  };
}
