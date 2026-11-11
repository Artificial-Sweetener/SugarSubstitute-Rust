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
/** Registry-free domain transport executes only the sole mounted owner's admitted leaf and original focus capability. */
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { currentContextMenu, type ContextMenu } from './context-menu-state.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { currentFocusHandoff } from './focus-handoff.js';
import {
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
  type NativeEditContinuation,
} from './document-value-continuation.js';
import { activateSavedSegment } from './saved-segment-activation.js';
import { spellingProvider } from './spelling-provider.js';
import { ignoreDuplicate } from './duplicate-ignore.js';
import { executeTextMenu } from './text-menu.js';
import type { LoraMenuActions } from './lora-menu-actions.js';
import type { SceneQueueActions } from './scene-queue-actions.js';

/** NativeContextMenu alone captures/transfers focus, closes publication, and consumes its one edit registry. */
export interface ContextMenuActivation {
  readonly view: EditorView;
  readonly actions: LoraMenuActions;
  readonly sceneActions: SceneQueueActions;
  active(): boolean;
  /** The sole native invocation owner fences delegation after its actual close boundary. */
  currentInvocation(): boolean;
  close(restore: boolean): Transaction | null;
  handoff(): FocusHandoff;
  takeEdit(
    menu: ContextMenu,
    item: Extract<ContextMenuItem, { kind: 'source-edit' }>,
  ): NativeEditContinuation | null;
  changeRichRendering(enabled: boolean): void;
}
/** The close boundary may reenter; freshness must still hold before any domain delegation. */
function current(owner: ContextMenuActivation, menu: ContextMenu): boolean {
  return (
    owner.active() &&
    owner.currentInvocation() &&
    !owner.view.composing &&
    !owner.view.compositionStarted &&
    currentContextMenu(owner.view.state, menu)
  );
}
/** True records real delegation; expected stale refusal after close never becomes an accepted aggregate result. */
export async function executeContextMenuAction(
  owner: ContextMenuActivation,
  menu: ContextMenu,
  item: ContextMenuItem,
): Promise<boolean> {
  const view = owner.view;
  if (item.kind === 'schedule-lora') {
    const handoff = owner.handoff();
    try {
      owner.close(false);
      if (
        current(owner, menu) &&
        currentFocusHandoff(view, handoff) &&
        !view.state.readOnly &&
        menu.utilities
      ) {
        await menu.utilities.scheduleLora(handoff);
        return true;
      }
      handoff.cancel();
      return false;
    } catch (cause) {
      try {
        handoff.cancel();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'field-utility-retirement-failed');
      }
      throw cause;
    }
  }
  if (item.kind === 'save-segment' || item.kind === 'insert-segment')
    return activateSavedSegment(
      view,
      menu,
      item,
      () => owner.close(false),
      () => owner.active() && owner.currentInvocation(),
      owner.handoff(),
    );
  if (item.kind === 'source-edit') return activateSourceEdit(owner, menu, item);
  if (item.kind === 'rich-rendering') {
    owner.close(true);
    if (!current(owner, menu)) return false;
    owner.changeRichRendering(!item.checked);
    return true;
  }
  if (item.kind === 'spelling-action') {
    const provider = view.plugin(spellingProvider);
    owner.close(true);
    if (!provider || !current(owner, menu)) return false;
    await provider.operate(item.finding, item.action);
    return true;
  }
  if (item.kind === 'ignore-duplicate') {
    owner.close(true);
    if (!current(owner, menu)) return false;
    view.dispatch({
      effects: ignoreDuplicate.of(item.finding.ignoreKey),
      annotations: Transaction.addToHistory.of(false),
    });
    return true;
  }
  owner.close(true);
  if (!current(owner, menu)) return false;
  if (item.kind === 'text') {
    await executeTextMenu(view, menu.text, item);
    return true;
  }
  if (item.kind === 'queue-scene' && menu.queue) {
    await owner.sceneActions.execute(menu.queue);
    return true;
  }
  if (menu.actions) {
    await owner.actions.execute(menu.actions, item);
    return true;
  }
  return false;
}
/** Follow the genuine registry handle through the actual close transaction, then settle original focus with ordered causes. */
async function activateSourceEdit(
  owner: ContextMenuActivation,
  menu: ContextMenu,
  item: Extract<ContextMenuItem, { kind: 'source-edit' }>,
): Promise<boolean> {
  const continuation = owner.takeEdit(menu, item),
    handoff = owner.handoff();
  let failure: { readonly cause: unknown } | null = null,
    delegated = false;
  try {
    const closing = owner.close(false),
      advanced =
        continuation && closing ? followNativeEditContinuation(continuation, closing) : null;
    if (
      advanced?.status === 'ready' &&
      currentFocusHandoff(owner.view, handoff) &&
      current(owner, menu)
    ) {
      const accepted = nativeEditContinuationTransaction(owner.view.state, advanced.value);
      if (accepted.status === 'ready') {
        owner.view.dispatch(accepted.value);
        delegated = true;
      }
    }
  } catch (cause) {
    failure = { cause };
  }
  try {
    if (owner.active() && currentFocusHandoff(owner.view, handoff)) await handoff.restore();
    else handoff.cancel();
  } catch (cause) {
    if (failure)
      throw new AggregateError([failure.cause, cause], 'native-menu-activation-cleanup-failed');
    throw cause;
  }
  if (failure) throw failure.cause;
  return delegated;
}
