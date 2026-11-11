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
/** One finite mounted surface owns CodeMirror view and measured focus lifetimes, including complete native input retirement. */
import { nativeFocusHandoff, type FocusHandoffPurpose } from './focus-handoff.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { StateEffect, Transaction, type EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { carryDuplicateIgnores, beginDuplicateIgnoreSession } from './duplicate-ignore.js';
import { NativeFocus } from './native-focus.js';
import { historyTransactions } from './prompt-history.js';
import { retireLoraMedia } from './lora-media-state.js';
import { retireRichProjection } from './projection-exposure.js';
import { initializeNativeStructure } from './structure.js';
import { compositionRetirement } from './composition-history.js';

/** Bind native selection to the actual DOM root, including light-DOM editors assigned to component slots. */
function createView(
  parent: HTMLElement,
  state: EditorState,
  scrollTo?: ReturnType<EditorView['scrollSnapshot']>,
): EditorView {
  const root = parent.getRootNode();
  return new EditorView({
    parent,
    // CodeMirror's inferred root follows assigned slots; native selection belongs to the light-DOM root.
    root: root instanceof ShadowRoot ? root : parent.ownerDocument,
    state: initializeNativeStructure(state),
    ...(scrollTo ? { scrollTo } : {}),
    /** Publish source and associated native history selection in one supported view update. */
    dispatchTransactions: (transactions, view) => view.update(historyTransactions(transactions)),
  });
}

/** Supported view construction seam keeps native lifetime transitions testable without a browser or a second editing owner. */
export type NativeViewFactory = typeof createView;

/** Retain one current native view; replacing its input lifetime carries the same immutable document, selection, configuration and history state. */
export class NativeSurface {
  private readonly parent: HTMLElement;
  private readonly create: NativeViewFactory;
  private current: EditorView;
  private focusOwner: NativeFocus;
  private readonly retirement: (current: () => boolean) => void;
  /** Prevent a focusout teardown from destroying a view already inside its retirement destroy call. */
  private retiringView: EditorView | null = null;
  private disposed = false;
  private intent = 0;
  /** Source-neutral callbacks supersede panel replies while inheriting the current input restoration destination. */
  private inputIntent = 0;
  private readonly handoffs = new WeakMap<
    FocusHandoff,
    {
      readonly current: () => boolean;
      readonly refocus: () => Promise<boolean>;
    }
  >();

  /** Bind one native surface and measured focus owner to the host's mounted element. */
  constructor(
    parent: HTMLElement,
    state: EditorState,
    publish: () => void,
    create: NativeViewFactory = createView,
  ) {
    this.parent = parent;
    this.create = create;
    this.retirement = (current) => this.retireInput(current, publish);
    this.current = this.create(parent, this.bindRetirement(beginDuplicateIgnoreSession(state)));
    this.focusOwner = new NativeFocus(this.current);
  }

  /** Consumers resolve the current view at execution rather than retaining a retired platform input lifetime. */
  get view(): EditorView {
    if (this.disposed) throw new Error('Native surface disposed');
    return this.current;
  }

  /** Focus readiness belongs to the current native view; optional caller cancellation never invalidates a newer surface intent. */
  focus(
    viewport: 'reveal-selection' | 'preserve-viewport' = 'reveal-selection',
    signal?: AbortSignal,
  ): Promise<boolean> {
    if (signal?.aborted) return Promise.resolve(false);
    this.intent++;
    this.inputIntent++;
    return this.focusOwner.focus(viewport, signal);
  }

  /** Capture one current source-free handoff before panel callbacks can choose another destination. */
  handoff(purpose: FocusHandoffPurpose = 'panel'): FocusHandoff {
    this.focusOwner.retire();
    this.inputIntent++;
    const intent = ++this.intent,
      inputIntent = this.inputIntent,
      view = this.view,
      active = view.root.activeElement;
    let consumed = false;
    const current = (): boolean =>
      !consumed &&
      !this.disposed &&
      (purpose !== 'panel' ? inputIntent === this.inputIntent : intent === this.intent) &&
      this.current === view;
    /** One destination/currentness policy serves initial pointer focus and one-shot later restoration. */
    const focus = (
      viewport: 'reveal-selection' | 'preserve-viewport' | undefined,
      consume: boolean,
    ): Promise<boolean> => {
      if (
        viewport !== undefined &&
        viewport !== 'reveal-selection' &&
        viewport !== 'preserve-viewport'
      )
        throw new TypeError('Invalid focus viewport intent');
      if (!current()) return Promise.resolve(false);
      if (consume) consumed = true;
      const destination = view.root.activeElement;
      if (
        destination &&
        destination !== active &&
        destination !== this.parent.ownerDocument.body &&
        !view.dom.contains(destination)
      )
        return Promise.resolve(false);
      return this.focusOwner.focus(viewport);
    };
    const handoff: FocusHandoff = {
      /** A later host intent, view replacement or external destination supersedes this reply. */
      restore: (viewport) => focus(viewport, true),
      /** Cancellation cannot retire a newer handoff. */
      cancel: () => {
        if (consumed) return;
        consumed = true;
        if (intent === this.intent) this.retireHandoff();
      },
    };
    this.handoffs.set(handoff, {
      current,
      refocus: () => focus('preserve-viewport', false),
    });
    return handoff;
  }
  /** An accepted source edit may continue only its own same-surface live handoff; ordinary host edits supersede panel and input restoration. */
  admitSource(continuation?: FocusHandoff): boolean {
    if (continuation !== undefined) return this.currentHandoff(continuation);
    this.retireHandoff();
    return true;
  }
  /** Validate a supplied continuation without retiring focus or consuming a prepared source handle. */
  currentHandoff(continuation: FocusHandoff): boolean {
    const current = this.handoffs.get(continuation);
    if (!current) throw new TypeError('Foreign focus continuation');
    return current.current();
  }
  /** Preserve one admitted invocation for later action transfer while focusing its original pointer destination without scrolling. */
  refocusHandoff(continuation: FocusHandoff): Promise<boolean> {
    const admission = this.handoffs.get(continuation);
    if (!admission) throw new TypeError('Foreign focus continuation');
    return admission.refocus();
  }
  /** Supersede panel replies and readiness; source-neutral selection/permission retains native input restoration, never a delayed panel reply. */
  retireHandoff(admission: 'preserve-input' | 'replace-input' = 'replace-input'): void {
    this.intent++;
    if (admission === 'replace-input') this.inputIntent++;
    this.focusOwner.retire();
  }

  /** Baseline restoration retires outstanding focus readiness before reinitializing native plugins. */
  setState(state: EditorState, continuation?: FocusHandoff): EditorState {
    if (continuation === undefined) this.retireHandoff();
    else if (!this.admitSource(continuation)) return this.view.state;
    const bound = this.bindRetirement(carryDuplicateIgnores(this.view.state, state));
    this.view.setState(bound);
    return bound;
  }

  /** Baselines install this surface's one finite callback; unchanged native state retains its actual owner without accumulating extensions. */
  private bindRetirement(state: EditorState): EditorState {
    const owner = state.facet(compositionRetirement);
    if (owner === this.retirement) return state;
    if (owner) throw new Error('Foreign native input retirement owner');
    return state.update({
      effects: StateEffect.appendConfig.of([
        compositionRetirement.of(this.retirement),
        nativeFocusHandoff.of({
          capture: (purpose) => this.handoff(purpose),
          current: (handoff) => this.currentHandoff(handoff),
          refocus: (handoff) => this.refocusHandoff(handoff),
        }),
      ]),
      annotations: Transaction.addToHistory.of(false),
    }).state;
  }

  /** Settle the entire startup sequence before retiring missing platform completion; new input, baseline and teardown invalidate publication. */
  retireInput(current: () => boolean, publish: () => void): void {
    const expected = this.view;
    expected.requestMeasure({
      key: this,
      /** CodeMirror first flushes native DOM input; capture its scroll snapshot in the native geometry read phase. */
      read: () =>
        !this.disposed && this.current === expected && current() ? expected.scrollSnapshot() : null,
      /** Retire input and publish outside paint, after rechecking newer native or host intent. */
      write: (ready) => {
        if (ready)
          queueMicrotask(() => {
            if (
              !this.disposed &&
              this.current === expected &&
              current() &&
              this.restartInput(ready)
            )
              publish();
          });
      },
    });
  }

  /** Retire the missing platform sequence once; return false if a synchronous host callback disposed the surface. */
  private restartInput(scrollTo: ReturnType<EditorView['scrollSnapshot']>): boolean {
    if (this.disposed) return false;
    const previous = this.view,
      focused = previous.hasFocus,
      inputIntent = this.inputIntent,
      root = previous.root;
    this.retiringView = previous;
    try {
      this.focusOwner.dispose();
      previous.destroy();
      if (this.disposed) return false;
      const state = retireLoraMedia(previous.state).update({
        effects: retireRichProjection.of(null),
        annotations: Transaction.addToHistory.of(false),
      }).state;
      const replacement = this.create(this.parent, state, scrollTo);
      if (this.disposed) {
        replacement.destroy();
        return false;
      }
      this.current = replacement;
      this.focusOwner = new NativeFocus(replacement);
      this.retiringView = null;
    } catch (cause) {
      this.disposed = true;
      throw cause;
    } finally {
      if (this.retiringView === previous) this.retiringView = null;
    }
    const active = root.activeElement;
    if (!this.disposed && focused && (!active || active === this.parent.ownerDocument.body))
      if (inputIntent === this.inputIntent) void this.focusOwner.focus();
    return !this.disposed;
  }

  /** Settle focus requests and retire all native plugins, listeners, DOM and measurement work once. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.intent++;
    this.inputIntent++;
    if (this.retiringView === this.current) return;
    this.focusOwner.dispose();
    this.current.destroy();
  }
}
