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
/** Actual Core proposals and application holds share the existing native Stage/Presentation fixture without a second driver. */
import assert from 'node:assert/strict';
import type { EditorState } from '@codemirror/state';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  reorderPreview,
  publishReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Preserve the existing fixture's genuine noninitial Core pickup, then publish a distinct pointer proposal. */
export function wholeFixture(
  prepareDocument?: (document: Document) => void,
  initialState?: EditorState,
) {
  const f = motionPresentationFixture(initialState, prepareDocument);
  const pickup = f.state.field(reorderPreview).value!,
    hold = createReorderApplicationHold(pickup.order);
  const order = moveReorderChip(pickup.capture.prepared, pickup.order, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  });
  assert.ok(order);
  assert.equal(
    publishReorderPreview(f.view, { ...pickup, order }, 'pointer', hold, () => true),
    'published',
  );
  const state = f.state.field(reorderPreview);
  assert.equal(new ReorderWidget(state.paint!, null).updateDOM(f.root, f.view), true);
  Object.assign(f.input, { order, pointer: 1 });
  return Object.assign(f, { hold });
}

/** A later Core proposal retains the same actual application hold and immutable native anchor. */
export function publishWhole(f: ReturnType<typeof wholeFixture>, insertion: number): void {
  const preview = f.state.field(reorderPreview).value!,
    order = moveReorderChip(preview.capture.prepared, preview.order, 0, {
      kind: 'row',
      row: 0,
      insertion,
    });
  assert.ok(order);
  assert.equal(
    publishReorderPreview(f.view, { ...preview, order }, 'pointer', f.hold, () => true),
    'published',
  );
  Object.assign(f.input, { order });
}
