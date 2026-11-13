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
/** One finite native menu owner measures prepared domain intents and delegates accepted actions. */
import { spellingProvider } from './spelling-provider.js';
import { executeContextMenuAction } from './context-menu-activation.js';
import { currentContextMenuAction } from './context-menu-activation-admission.js';
import {
  requestLocalMenu,
  openLocalPointerMenu,
  localMenuPointerTarget,
  localMenuKeyboardIntent,
  localMenuContainsEventTarget,
} from './context-menu-local-presentation.js';
import { requestFieldMenu, type FieldMenuSettlement } from './context-menu-field-presentation.js';
import { captureFieldActionContext } from './field-actions-projection.js';
import type {
  FieldActionContext,
  FieldActionOutcome,
  FieldActionPreparation,
} from './field-actions-contracts.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { savedSegments } from './saved-segment-menu.js';
import { captureFocusHandoff, currentFocusHandoff } from './focus-handoff.js';
import { Transaction, type EditorState } from '@codemirror/state';
import { type EditorView, type ViewUpdate } from '@codemirror/view';
import { LoraMenuActions } from './lora-menu-actions.js';
import { contextMenuTargetAt, type MenuTarget } from './context-menu-preparation.js';
import { SceneQueueActions } from './scene-queue-actions.js';
import { sceneQueuePublication } from './scene-queue.js';
import { editorFreshness } from './freshness.js';
import { loraPresentation } from './lora-presentation.js';
import { loraPageHost, loraMetadataHost } from './lora-action-hosts.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { installContextMenu, contextMenu, type ContextMenu } from './context-menu-state.js';
import type { ContextMenuItem } from './context-menu-contracts.js';

import { isolateHistory } from '@codemirror/commands';
import { ContextMenuEdits } from './context-menu-edits.js';

/** A pending native opening owns its clicked logical-document UTF-16 position and optional viewport point until presentation publishes. */
interface MenuOpening {
  readonly position: number;
  readonly point: { readonly x: number; readonly y: number } | undefined;
}

/** One mounted view owns outside/viewport listeners, pending measurement and delegated LoRA work. */
export class NativeContextMenu {
  private disposed = false;
  private generation = 0;
  private closeQueued = false;
  private opening: MenuOpening | null = null;
  private invocationHandoff: FocusHandoff | null = null;
  private fieldSettlement: FieldMenuSettlement | null = null;
  private readonly actions: LoraMenuActions;
  private readonly sceneActions: SceneQueueActions;
  private readonly view: EditorView;
  private readonly ownerWindow: Window;
  private readonly edits = new ContextMenuEdits();
  private readonly changeRichRendering: (enabled: boolean) => void;

  /** Bind finite listeners and the existing external action lane to this real editor lifetime. */
  constructor(view: EditorView, changeRichRendering: (enabled: boolean) => void) {
    this.view = view;
    this.changeRichRendering = changeRichRendering;
    const ownerWindow = view.dom.ownerDocument.defaultView;
    if (!ownerWindow) throw new Error('Native editor window unavailable');
    this.ownerWindow = ownerWindow;
    this.actions = new LoraMenuActions(view);
    this.sceneActions = new SceneQueueActions(view);
    view.dom.ownerDocument.addEventListener('pointerdown', this.outside);
    ownerWindow.addEventListener('scroll', this.viewport, true);
    ownerWindow.addEventListener('resize', this.viewport);
  }

  /** Source, host, catalog and composition retirement reject pending placement and cancel delegated work. */
  update(update: ViewUpdate): void {
    this.edits.follow(update.transactions);
    if (!update.state.field(contextMenu)) {
      this.edits.retireVisible();
      this.view.plugin(spellingProvider)?.cancelSuggestions();
      if (!this.opening) this.retireInvocation('cancelled');
    }
    this.actions.update(update);
    this.sceneActions.update(update);
    if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      !update.startState.selection.eq(update.state.selection) ||
      this.view.composing ||
      this.view.compositionStarted
    ) {
      this.generation++;
      this.opening = null;
      this.edits.cancelPending();
      this.retireInvocation('cancelled');
    } else if (
      update.startState.field(loraPresentation) !== update.state.field(loraPresentation) ||
      update.startState.facet(loraPageHost) !== update.state.facet(loraPageHost) ||
      update.startState.facet(loraMetadataHost) !== update.state.facet(loraMetadataHost) ||
      update.startState.facet(wildcardCatalog) !== update.state.facet(wildcardCatalog) ||
      update.startState.field(sceneQueuePublication) !==
        update.state.field(sceneQueuePublication) ||
      update.startState.field(savedSegments, false) !== update.state.field(savedSegments, false)
    ) {
      this.generation++;
      this.edits.cancelPending();
      const opening = this.opening;
      if (
        this.fieldSettlement ||
        update.startState.field(contextMenu)?.surface === 'field' ||
        update.state.field(contextMenu)?.surface === 'field'
      ) {
        this.opening = null;
        this.retireInvocation('cancelled');
      } else if (opening)
        queueMicrotask(() => {
          if (this.disposed || this.opening !== opening) return;
          const target = contextMenuTargetAt(this.view.state, opening.position);
          if (target) this.request(target, opening.point);
        });
    }
    if (
      (update.viewportChanged && update.state.field(contextMenu)?.surface !== 'field') ||
      this.view.composing ||
      this.view.compositionStarted
    )
      this.queueClose();
  }

  /** Native pointer admission delegates original focus before invocation callbacks and preserves directional selection. */
  context(event: MouseEvent): boolean {
    const target = localMenuPointerTarget(this.view, event);
    return target
      ? openLocalPointerMenu(this.view, target, event, (current, point, handoff) =>
          this.request(current, point, handoff),
        )
      : false;
  }

  /** Preserve native selection before browser right-button caret handling on an eligible target. */
  pointer(event: MouseEvent): boolean {
    if (event.button !== 2 || !localMenuPointerTarget(this.view, event)) return false;
    event.preventDefault();
    return true;
  }

  /** Escape retires pending/visible intent before tooltip focus; opening keys use native caret and prepared eligibility. */
  key(event: KeyboardEvent): boolean {
    const intent = localMenuKeyboardIntent(event);
    if (
      intent === 'dismiss' &&
      !this.disposed &&
      (this.opening || this.view.state.field(contextMenu))
    ) {
      event.preventDefault();
      this.close(true);
      return true;
    }
    if (intent !== 'open') return false;
    if (this.view.composing || this.view.compositionStarted) return false;
    const target = contextMenuTargetAt(this.view.state, this.view.state.selection.main.head);
    if (!target || !this.request(target)) return false;
    event.preventDefault();
    return true;
  }

  /** Capture before supersession callbacks; both surfaces retain the same source-free capability and failure policy. */
  private beginInvocation(state: EditorState, supplied?: FocusHandoff): FocusHandoff | null {
    const capturedGeneration = this.generation;
    const handoff = supplied ?? captureFocusHandoff(this.view, 'reserved-source');
    if (capturedGeneration !== this.generation) {
      handoff.cancel();
      return null;
    }
    try {
      this.close(false);
    } catch (cause) {
      try {
        handoff.cancel();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'native-menu-capture-retirement-failed');
      }
      throw cause;
    }
    if (
      this.disposed ||
      this.generation > capturedGeneration + 1 ||
      state.field(editorFreshness) !== this.view.state.field(editorFreshness) ||
      !state.selection.eq(this.view.state.selection) ||
      !currentFocusHandoff(this.view, handoff)
    ) {
      handoff.cancel();
      return null;
    }
    this.invocationHandoff = handoff;
    return handoff;
  }

  /** Capture local invocation authority once; the presentation collaborator owns preparation and measurement only. */
  private request(
    target: MenuTarget,
    point?: { readonly x: number; readonly y: number },
    supplied?: FocusHandoff,
  ): boolean {
    if (this.disposed || this.view.composing || this.view.compositionStarted) return false;
    const state = this.view.state;
    const handoff = this.beginInvocation(state, supplied);
    if (!handoff) return false;
    const freshness = state.field(editorFreshness),
      selection = state.selection,
      generation = ++this.generation;
    this.opening = { position: target.position, point };
    const current = () =>
      !this.disposed &&
      !this.closeQueued &&
      generation === this.generation &&
      freshness === this.view.state.field(editorFreshness) &&
      selection.eq(this.view.state.selection) &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      currentFocusHandoff(this.view, handoff);
    requestLocalMenu(this.view, target, point, {
      measurementKey: this,
      current,
      prepare: (state, menu) => current() && this.edits.prepare(state, menu),
      publish: (menu) => current() && this.publish(menu),
      complete: () => {
        if (generation === this.generation) this.opening = null;
      },
      cancel: () => {
        if (generation === this.generation) this.close(false);
      },
    });
    return true;
  }

  /** Every surface uses the one edit registry and source-neutral native publication transport. */
  private publish(menu: ContextMenu): boolean {
    if (!this.edits.publish(menu)) return false;
    this.view.dispatch(
      this.view.state.update({
        effects: installContextMenu.of(menu),
        annotations: [Transaction.addToHistory.of(false), isolateHistory.of('full')],
      }),
    );
    return this.view.state.field(contextMenu) === menu;
  }

  /** This mounted contribution remains available in read-only mode for rendering intent. */
  fieldActionsAvailable(): boolean {
    return !this.disposed;
  }

  /** Capture selection and original focus before host menu effects; semantic preparation runs outside input. */
  prepareFieldActions(context: FieldActionContext): Promise<FieldActionPreparation> {
    const observedGeneration = this.generation;
    const anchor = captureFieldActionContext(context);
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    if (observedGeneration !== this.generation) return Promise.resolve({ status: 'cancelled' });
    if (this.view.composing || this.view.compositionStarted)
      return Promise.resolve({ status: 'unavailable', reason: 'composing' });
    const state = this.view.state,
      selection = state.selection,
      freshness = state.field(editorFreshness);
    const handoff = this.beginInvocation(state);
    if (!handoff)
      return Promise.resolve({
        status: this.disposed ? 'disposed' : 'cancelled',
      });
    const generation = ++this.generation;
    this.opening = { position: selection.main.head, point: undefined };
    const current = () =>
      !this.disposed &&
      generation === this.generation &&
      freshness === this.view.state.field(editorFreshness) &&
      selection.eq(this.view.state.selection) &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      currentFocusHandoff(this.view, handoff);
    return new Promise((resolve, reject) => {
      const settlement = { resolve, reject };
      this.fieldSettlement = settlement;
      requestFieldMenu(this.view, anchor, selection.main.head, settlement, {
        current,
        prepare: (state, menu) => this.edits.prepare(state, menu),
        publish: (menu) => {
          this.opening = null;
          return this.publish(menu);
        },
        detach: () => {
          if (this.fieldSettlement === settlement) this.fieldSettlement = null;
        },
        retire: () => {
          if (generation === this.generation && this.fieldSettlement === settlement)
            this.close(false);
          else settlement.resolve({ status: 'cancelled' });
        },
        fail: () => {
          if (generation === this.generation) this.close(false);
        },
        activate: (menu, item) => this.activateField(menu, item),
        release: (menu) => {
          if (this.view.state.field(contextMenu) === menu) this.close(false);
        },
      });
    });
  }

  /** Bound presentation never grants another leaf identity or bypasses common installed-menu admission. */
  private async activateField(
    menu: ContextMenu,
    item: ContextMenuItem,
  ): Promise<FieldActionOutcome> {
    if (this.disposed) return { status: 'refused', reason: 'disposed' };
    if (this.view.composing || this.view.compositionStarted)
      return { status: 'refused', reason: 'composing' };
    if (item.kind === 'disabled') return { status: 'refused', reason: 'disabled' };
    if (
      !this.invocationHandoff ||
      !currentFocusHandoff(this.view, this.invocationHandoff) ||
      !currentContextMenuAction(this.view.state, menu, item)
    )
      return { status: 'refused', reason: 'stale' };
    return (await this.activate(menu, item))
      ? { status: 'accepted' }
      : { status: 'refused', reason: 'stale' };
  }

  /** Transfer the one original invocation capability to its accepted action without recapturing popup focus. */
  private takeHandoff(): FocusHandoff {
    const handoff = this.invocationHandoff;
    if (!handoff) throw new Error('native-menu-invocation-handoff-missing');
    this.invocationHandoff = null;
    return handoff;
  }

  /** Pending preparation and retained focus settle exactly once on the sole invocation's retirement. */
  private retireInvocation(status: 'cancelled' | 'disposed'): void {
    const handoff = this.invocationHandoff,
      settlement = this.fieldSettlement;
    this.invocationHandoff = null;
    this.fieldSettlement = null;
    try {
      handoff?.cancel();
    } catch (cause) {
      settlement?.reject(cause);
      throw cause;
    }
    settlement?.resolve({ status });
  }
  /** Explicit dismissal restores native focus and returns its actual metadata transaction for an already accepted edit. */
  close(restoreFocus: boolean): Transaction | null {
    if (this.disposed) return null;
    const menu = this.view.state.field(contextMenu);
    if (!menu && !this.opening && !this.invocationHandoff && !this.fieldSettlement) return null;
    const handoff = menu && restoreFocus ? this.takeHandoff() : undefined;
    const generation = ++this.generation;
    this.opening = null;
    let closing: Transaction | null = null;
    const causes: unknown[] = [];
    const steps = [
      () => this.retireInvocation('cancelled'),
      () => this.view.plugin(spellingProvider)?.cancelSuggestions(),
      () => this.edits.clear(),
      () => {
        if (menu && generation === this.generation && this.view.state.field(contextMenu) === menu) {
          closing = this.view.state.update({
            effects: installContextMenu.of(null),
            annotations: Transaction.addToHistory.of(false),
          });
          this.view.dispatch(closing);
        }
      },
      () => {
        if (!this.disposed) void handoff?.restore();
        else handoff?.cancel();
      },
    ];
    for (const step of steps) {
      try {
        step();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'native-menu-retirement-failed');
    return closing;
  }

  /** Only exact enabled action leaves can execute; disabled diagnostic rows never dispatch an edit or external intent. */
  async activate(menu: ContextMenu, item: ContextMenuItem): Promise<boolean> {
    if (
      this.disposed ||
      this.view.composing ||
      this.view.compositionStarted ||
      !currentContextMenuAction(this.view.state, menu, item)
    )
      return false;
    const issuedGeneration = this.generation;
    let closedGeneration: number | null = null;
    return executeContextMenuAction(
      {
        view: this.view,
        actions: this.actions,
        sceneActions: this.sceneActions,
        active: () => !this.disposed,
        currentInvocation: () => closedGeneration !== null && this.generation === closedGeneration,
        close: (restore) => {
          if (this.generation !== issuedGeneration) return null;
          const closing = this.close(restore);
          if (this.generation === issuedGeneration + 1) closedGeneration = this.generation;
          return closing;
        },
        handoff: () => this.takeHandoff(),
        takeEdit: (menu, leaf) => this.edits.take(menu, leaf),
        changeRichRendering: (enabled) => this.changeRichRendering(enabled),
      },
      menu,
      item,
    );
  }
  /** Coalesce viewport/composition dismissal outside native update; a replacement menu retains its own lifetime. */
  private queueClose(): void {
    if (this.closeQueued || !this.view.state.field(contextMenu)) return;
    this.closeQueued = true;
    const menu = this.view.state.field(contextMenu);
    queueMicrotask(() => {
      this.closeQueued = false;
      if (!this.disposed && this.view.state.field(contextMenu) === menu) this.close(false);
    });
  }

  /** An outside pointer closes the overlay without caret movement or focus capture. */
  private readonly outside = (event: PointerEvent): void => {
    if (this.fieldSettlement || this.view.state.field(contextMenu)?.surface === 'field') return;
    if (localMenuContainsEventTarget(event)) return;
    this.close(false);
  };

  /** Captured screen coordinates retire on viewport movement rather than following stale geometry. */
  private readonly viewport = (event: Event): void => {
    if (this.view.state.field(contextMenu)?.surface === 'field') return;
    if (localMenuContainsEventTarget(event)) return;
    if (this.view.state.field(contextMenu)) this.close(false);
  };

  /** Release listeners, pending publication and delegated work before native teardown. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.generation++;
    this.opening = null;
    const causes: unknown[] = [];
    const steps = [
      () => this.retireInvocation('disposed'),
      () => this.view.plugin(spellingProvider)?.cancelSuggestions(),
      () => this.edits.clear(),
      () => this.actions.dispose(),
      () => this.sceneActions.dispose(),
      () => this.view.dom.ownerDocument.removeEventListener('pointerdown', this.outside),
      () => this.ownerWindow.removeEventListener('scroll', this.viewport, true),
      () => this.ownerWindow.removeEventListener('resize', this.viewport),
    ];
    for (const step of steps) {
      try {
        step();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'native-menu-destruction-failed');
  }
}
