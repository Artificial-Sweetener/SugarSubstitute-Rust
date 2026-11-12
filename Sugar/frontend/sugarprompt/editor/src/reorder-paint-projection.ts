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
/** The complete paint adapter borrows either genuine native pickup marks or the canonical mounted Widget. */
import type { EditorView } from '@codemirror/view';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
import { reorderPreview } from './reorder-preview.js';
import { currentReorderPaintTargets, currentReorderPaintLease } from './reorder-widget.js';
import {
  admitNativeReorderProjection,
  currentNativeReorderProjectionTargets,
} from './reorder-native-projection.js';

/** Semantic publication admission borrows the actual pickup painter's complete mounted issue. */
function currentReorderWidgetProjectionLease(
  view: EditorView,
  preview: ReorderPreview,
): (() => boolean) | null {
  const publication = view.state.field(reorderPreview, false);
  if (publication?.value !== preview || !publication.paint) return null;
  const projection = publication.paint,
    lease = currentReorderPaintLease(view, projection);
  if (!lease) return null;
  return () =>
    view.state.field(reorderPreview, false)?.value === preview &&
    view.state.field(reorderPreview, false)?.paint === projection &&
    lease() &&
    view.state.field(reorderPreview, false)?.value === preview &&
    view.state.field(reorderPreview, false)?.paint === projection;
}

/** READ admits one actual projection issue; semantic proposal identity remains distinct from the immutable pickup paint. */
export function currentReorderProjectionLease(
  view: EditorView,
  preview: ReorderPreview,
): (() => boolean) | null {
  const canonical = currentReorderWidgetProjectionLease(view, preview);
  return (
    canonical ??
    (view.state.field(reorderPreview, false)?.native
      ? admitNativeReorderProjection(view, preview)
      : null)
  );
}
/** Current target identity is borrowed without discovery, layout, copies or semantic traversal. */
export function currentReorderProjectionTargets(
  view: EditorView,
): readonly MountedReorderTarget[] | null {
  const publication = view.state.field(reorderPreview, false),
    canonical = currentReorderPaintTargets(view);
  if (!publication?.native) return canonical;
  if (canonical && publication.paint && currentReorderPaintLease(view, publication.paint)?.())
    return canonical;
  return currentNativeReorderProjectionTargets(view);
}
