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
/** Source-neutral retirement clears only the session-issued preview and cursor still installed on this native view. */
import { snapshotPresentation } from './native-snapshot-policy.js';
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { NativeReorderEntry } from './reorder-entry.js';
import { installReorderPreview, reorderPreview } from './reorder-preview.js';
import { reorderGrabCursor, setReorderGrabCursor } from './reorder-grab-cursor.js';

/** Exact old preview/cursor cleanup survives owner disposal and cannot remove a newer issued identity. */
export function retireReorderPublication(
  view: EditorView,
  token: object | null,
  cursor: object | null,
  defer: boolean,
): void {
  if (!token && !cursor) return;
  const clear = (): void => {
    const cursorEffect =
      cursor && view.state.field(reorderGrabCursor, false) === cursor
        ? setReorderGrabCursor.of(null)
        : null;
    const previewEffect =
      token && view.state.field(reorderPreview, false)?.value?.token === token
        ? installReorderPreview.of(null)
        : null;
    if (!cursorEffect && !previewEffect) return;
    view.dispatch({
      effects: [...(cursorEffect ? [cursorEffect] : []), ...(previewEffect ? [previewEffect] : [])],
      annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
    });
  };
  if (defer) queueMicrotask(clear);
  else clear();
}

/** A session marks itself disposed first; its complete entry and measured presentation boundary then retires even after close failure. */
export function destroyReorderSessionOwners(
  close: () => void,
  entry: NativeReorderEntry,
  disposeMeasurement: () => void,
): void {
  const failures: unknown[] = [];
  for (const retire of [close, () => entry.destroy(), disposeMeasurement]) {
    try {
      retire();
    } catch (cause) {
      failures.push(cause);
    }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'Reorder session disposal failed');
}
