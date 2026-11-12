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
/** Saved-segment activation delegates admitted native targets to canonical ingestion and the mounted modal owner. */
import type { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { currentContextMenu, type ContextMenu } from './context-menu-state.js';
import { currentFocusHandoff } from './focus-handoff.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { enteredTextInsertion } from './ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { savedSegmentActions } from './saved-segment-actions.js';
import { spellingLexicon } from './spelling-provider.js';

/** The common menu owner admits the genuine visible leaf before delegation; this transport owns no registry or target state. */
export async function activateSavedSegment(
  view: EditorView,
  menu: ContextMenu,
  item: Extract<ContextMenuItem, { kind: 'save-segment' | 'insert-segment' }>,
  close: () => unknown,
  isActive: () => boolean,
  handoff: FocusHandoff,
): Promise<boolean> {
  let delegated = false;
  let transferred = false;
  let failure: { readonly cause: unknown } | null = null;
  try {
    close();
    if (
      isActive() &&
      currentContextMenu(view.state, menu) &&
      currentFocusHandoff(view, handoff) &&
      !view.state.readOnly &&
      !view.composing &&
      !view.compositionStarted
    ) {
      if (item.kind === 'save-segment') {
        const owner = view.plugin(savedSegmentActions);
        if (owner) {
          owner.save(menu.presets, handoff);
          transferred = true;
          delegated = true;
        }
      } else {
        dispatchNativeIngestion(
          view,
          enteredTextInsertion(
            view.state,
            menu.presets.from,
            menu.presets.to,
            item.segment.text,
            view.state.facet(spellingLexicon)(),
          ),
        );
        delegated = true;
      }
    }
  } catch (cause) {
    failure = { cause };
  }
  try {
    if (!transferred) {
      if (item.kind === 'save-segment') handoff.cancel();
      else await handoff.restore();
    }
  } catch (cause) {
    if (failure)
      throw new AggregateError(
        [failure.cause, cause],
        'saved-segment-activation-retirement-failed',
      );
    throw cause;
  }
  if (failure) throw failure.cause;
  return delegated;
}
