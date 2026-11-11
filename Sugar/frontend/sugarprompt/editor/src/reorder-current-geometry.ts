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
/** One current-view adapter selects Core marker witnesses before native observation and routes full geometry to the existing mounted owner. */
import { requireValue } from '../../core/src/required.js';
import { nativeReorderRecipe, nativeReorderElement } from './reorder-native-recipe.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import { reorderPreview } from './reorder-preview.js';
import { measuredReorderWidget, measuredReorderMarker } from './reorder-widget.js';
import {
  reorderMarkerWitnesses,
  selectReorderMarkerTargets,
  type ReorderRect,
} from './reorder-geometry.js';
/** Keyboard and full native geometry remain complete; pointer marker paint supplies its exact current semantic input. */
export type CurrentReorderGeometryRequest =
  | {
      readonly kind: 'complete';
    }
  | {
      readonly kind: 'marker';
      readonly input: ReorderMeasureInput;
    };
/** Read only in the caller's supported CM measure phase; native and changed-order paint share semantic witness selection without another layout cache. */
export function readCurrentReorderGeometry(
  view: EditorView,
  request: CurrentReorderGeometryRequest,
): readonly ReorderRect[] {
  const witnesses =
    request.kind === 'marker' && request.input.hidden
      ? reorderMarkerWitnesses(request.input.order, request.input.hidden, request.input.held)
      : null;
  if (request.kind === 'marker' && !witnesses) return [];
  const preview = view.state.field(reorderPreview, false);
  // An anchor is real native paint, but cannot certify a different semantic endpoint's marker.
  if (preview?.pointer?.mode === 'stage' && request.kind === 'marker') return [];
  if (preview?.value && !preview.native)
    return (
      (witnesses
        ? measuredReorderMarker(
            view,
            requireValue(preview.paint, 'reorder-current-geometry'),
            witnesses,
          )
        : measuredReorderWidget(view, requireValue(preview.paint, 'reorder-current-geometry'))) ??
      []
    );
  const recipe =
    preview?.paint &&
    nativeReorderRecipe(preview.paint.capture.context.values, preview.paint.capture.target.entry);
  if (!recipe) return [];
  const result: ReorderRect[] = [];
  for (const node of view.contentDOM.querySelectorAll<HTMLElement>(
    '.sp-reorder-preview [data-reorder-chip], .sp-reorder-preview [data-reorder-gap], .sp-reorder-preview [data-reorder-partition]',
  )) {
    if (!nativeReorderElement(node, recipe.key)) continue;
    const kind =
        node.dataset.reorderChip !== undefined
          ? 'chip'
          : node.dataset.reorderPartition !== undefined
            ? 'partition'
            : 'gap',
      index = Number(
        kind === 'chip'
          ? node.dataset.reorderChip
          : kind === 'partition'
            ? node.dataset.reorderPartition
            : node.dataset.reorderGap,
      );
    if (
      witnesses &&
      !selectReorderMarkerTargets(
        [
          {
            kind,
            index,
            ...(node.dataset.reorderBlank !== undefined
              ? { blank: Number(node.dataset.reorderBlank) }
              : {}),
          },
        ],
        witnesses,
      ).length
    )
      continue;
    for (const rect of node.getClientRects())
      if (rect.width || rect.height)
        result.push({
          kind,
          index,
          ...(node.dataset.reorderBlank !== undefined
            ? { blank: Number(node.dataset.reorderBlank) }
            : {}),
          left: rect.left,
          right: rect.right,
          top: rect.top,
          bottom: rect.bottom,
        });
  }
  return result;
}
