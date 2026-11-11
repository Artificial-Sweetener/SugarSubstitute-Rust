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
/** One native menu state owns finite derived presentation across prepared action domains. */
import { currentSpellingMenu, type PreparedSpellingMenu } from './spelling-menu.js';
import { StateEffect, StateField, type EditorState } from '@codemirror/state';
import type { Rect } from '@codemirror/view';
import { currentDuplicateMenu, type PreparedDuplicateMenu } from './duplicate-menu.js';
import { currentLoraMenu, type PreparedLoraMenu } from './lora-menu-targets.js';
import { currentWildcardMenu, type PreparedWildcardMenu } from './wildcard-menu-target.js';
import { currentSceneQueueTarget, type PreparedSceneQueueTarget } from './scene-queue.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { currentSavedSegmentMenu, type PreparedSavedSegmentMenu } from './saved-segment-menu.js';
import { richRendering } from './presentation.js';
import { currentTextMenu, type PreparedTextMenu } from './text-menu.js';
import { fieldActionUtilities, type FieldActionUtilities } from './field-action-utilities.js';
import { currentDocumentMenu, type PreparedDocumentMenu } from './document-menu-target.js';

/** One prepared presentation composes independent domains without adopting their behavior ownership. */
export interface PreparedContextMenu {
  readonly surface: 'local' | 'field';
  readonly utilities: FieldActionUtilities | null;
  readonly scheduleItem: Extract<ContextMenuItem, { kind: 'schedule-lora' }> | null;
  readonly presets: PreparedSavedSegmentMenu;
  readonly text: PreparedTextMenu;
  readonly kind:
    | 'inline'
    | 'general'
    | 'wildcard'
    | 'unsupported-scene'
    | 'duplicate-segment'
    | 'spelling';
  readonly from: number;
  readonly to: number;
  readonly diagnostic:
    | PreparedWildcardMenu
    | PreparedDocumentMenu
    | PreparedDuplicateMenu
    | PreparedSpellingMenu
    | null;
  readonly actions: PreparedLoraMenu | null;
  readonly queue: PreparedSceneQueueTarget | null;
  readonly queueItem: Extract<ContextMenuItem, { kind: 'queue-scene' }> | null;
  /** The exact prepared checkbox observes the existing native configuration facet. */
  readonly renderingItem: Extract<ContextMenuItem, { kind: 'rich-rendering' }>;
  readonly items: readonly ContextMenuItem[];
}

/** One measured viewport anchor belongs to the current prepared menu intent. */
export type ContextMenu = PreparedContextMenu & { readonly anchor: Rect };

/** Delegate domain freshness and visibility without reproducing its behavior rules. */
export function currentContextMenu(state: EditorState, menu: PreparedContextMenu): boolean {
  return (
    state.facet(fieldActionUtilities) === menu.utilities &&
    currentTextMenu(state, menu.text) &&
    currentSavedSegmentMenu(state, menu.presets) &&
    state.facet(richRendering) === menu.renderingItem.checked &&
    (menu.diagnostic === null ||
      (menu.diagnostic.kind === 'wildcard'
        ? currentWildcardMenu(state, menu.diagnostic)
        : menu.diagnostic.kind === 'duplicate-segment'
          ? currentDuplicateMenu(state, menu.diagnostic)
          : menu.diagnostic.kind === 'spelling'
            ? currentSpellingMenu(state, menu.diagnostic)
            : currentDocumentMenu(state, menu.diagnostic))) &&
    (menu.actions === null || currentLoraMenu(state, menu.actions)) &&
    (menu.queue === null || currentSceneQueueTarget(state, menu.queue))
  );
}

/** Menu installation changes derived presentation only; source and native history remain untouched. */
export const installContextMenu = StateEffect.define<ContextMenu | null>();

/** Explicit selection and obsolete domain identities close presentation in the same native transaction. */
export const contextMenu = StateField.define<ContextMenu | null>({
  create: () => null,
  update(current, transaction) {
    if (current && (transaction.selection || !currentContextMenu(transaction.state, current)))
      current = null;
    for (const effect of transaction.effects)
      if (effect.is(installContextMenu)) {
        const menu = effect.value;
        current = menu && currentContextMenu(transaction.state, menu) ? menu : null;
      }
    return current;
  },
});
