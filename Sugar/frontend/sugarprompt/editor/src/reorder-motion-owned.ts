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
/** Captured terminal retirement owns no current Resource selection, adoption decision or second issue registry. */
import type { EditorView } from '@codemirror/view';
import type { ReorderPointerPaint } from './reorder-preview-contracts.js';
import type { ReorderMotionMask } from './reorder-motion-mask.js';
import { releaseReorderMotionMask } from './reorder-motion-mask.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import { reorderPreview } from './reorder-preview.js';
import { reorderPaintIssueName } from './reorder-paint-identity.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
import type { ReorderMotionStyles } from './reorder-motion-styles.js';
import type { ReorderMotionObservation } from './reorder-motion-observation.js';
import type { ReorderLandingPublication } from './reorder-landing-paint.js';
/** One exact acquisition retains only its own paint, native subscriptions and publication issue. */
export interface OwnedMotionResource {
  readonly issue: object;
  released: boolean;
  readonly pointer: ReorderPointerPaint | null;
  mask: ReorderMotionMask | null;
  readonly scene: ReorderMotionScene;
  readonly styles: ReorderMotionStyles;
  readonly lease: () => boolean;
  readonly admitted: () => boolean;
  observation: ReorderMotionObservation | null;
  readonly landing: ReorderLandingPublication | null;
}
/** Released-first exact cleanup attempts every captured native lifetime and returns flat ordered raw causes to Resource. */
export function releaseOwnedMotionResource(
  view: EditorView,
  owned: OwnedMotionResource,
  observe: <T>(action: () => T) => T,
): readonly unknown[] {
  if (owned.released) return [];
  owned.released = true;
  const failures: unknown[] = [];
  for (const release of [
    () => owned.landing?.release(),
    () => {
      if (owned.scene.node.dataset.reorderMotionEndpoint === reorderPaintIssueName(owned.issue))
        delete owned.scene.node.dataset.reorderMotionEndpoint;
    },
    () => {
      if (owned.scene.whole) owned.scene.destroy();
    },
    () => owned.styles.deactivate(owned.issue),
    () => {
      for (const cause of owned.observation?.disconnect() ?? []) failures.push(cause);
    },
    () => {
      if (!owned.scene.whole) owned.scene.destroy();
    },
    () =>
      queueMicrotask(() =>
        observe(() => {
          if (view.state.field(reorderPreview, false)?.mask?.issue === owned.issue)
            view.dispatch({
              effects: releaseReorderMotionMask.of(owned.issue),
              annotations: snapshotPresentation.of(true),
            });
        }),
      ),
  ])
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  return failures;
}
