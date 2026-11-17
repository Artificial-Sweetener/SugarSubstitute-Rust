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
/** Anchor the host's model gallery at the caret and bind it to the one native completion session. */
import { Transaction } from '@codemirror/state';
import type { EditorView, Rect, TooltipView, ViewUpdate } from '@codemirror/view';
import { completionSession, requestCompletionAcceptance } from './completion-session.js';
import { applyCompletionHeight, readCompletionHeight } from './completion-space.js';
import { captureFocusHandoff } from './focus-handoff.js';
import {
  modelCompletionPopupHost,
  type ModelCompletionArrow,
  type ModelCompletionPopup,
} from './model-completion-popup.js';
import { acceptCompletionOption } from './prompt-completion-acceptance.js';
import type { PreparedModelCompletions } from './prompt-completion-model.js';

/** The gallery each editor currently shows, for keys routed through the native keymap. */
const mounted = new WeakMap<EditorView, ModelCompletionPopup>();

/**
 * The candidate one arrow key away in the mounted gallery, or null while no gallery is mounted.
 *
 * Visual rows belong to the host's gallery, so the keymap asks it rather than laying tiles out.
 */
export function modelCompletionNeighbor(
  view: EditorView,
  selected: number,
  key: ModelCompletionArrow,
): number | null {
  return mounted.get(view)?.neighbor(selected, key) ?? null;
}

/** One chrome-free tooltip holds the host's surface; selection and acceptance stay in native state. */
class ModelCompletionTooltip implements TooltipView {
  readonly dom = document.createElement('div');
  /** Keep the completion gap from the caret line; CodeMirror chooses the side. */
  readonly offset = { x: 0, y: 6 } as const;
  /** The surface is bounded through its own height variable rather than native resizing. */
  readonly resize = false;
  private readonly view: EditorView;
  private readonly popup: ModelCompletionPopup;
  private space: Rect | null = null;
  private shown: PreparedModelCompletions | null = null;
  private selected = -1;
  private queued = false;
  private disposed = false;

  /** Mount the host's gallery once for this completion lifetime. */
  constructor(view: EditorView) {
    this.view = view;
    this.dom.className = 'sp-model-completions';
    const host = view.state.facet(modelCompletionPopupHost);
    if (!host) throw new Error('Model completions require a host gallery');
    this.popup = host.mount({ activate: (index) => this.activate(index) });
    this.dom.append(this.popup.element);
    this.dom.addEventListener('mousedown', this.pressed);
    this.dom.addEventListener('focusin', this.focused);
    mounted.set(view, this.popup);
    this.project();
  }

  /** Bound the surface once it is in the document. */
  mount(): void {
    this.measure();
  }

  /** Follow the session's ranked models and current candidate. */
  update(update: ViewUpdate): void {
    if (this.disposed) return;
    const before = update.startState.field(completionSession),
      after = update.state.field(completionSession);
    if (before?.result !== after?.result || before?.selected !== after?.selected) {
      this.project();
      this.measure();
    }
  }

  /** Retain the native available space; the caret geometry is read in a measurement phase. */
  positioned(space: Rect): void {
    this.space = { ...space };
    this.measure();
  }

  /** Release the gallery with the tooltip. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.dom.removeEventListener('mousedown', this.pressed);
    this.dom.removeEventListener('focusin', this.focused);
    if (mounted.get(this.view) === this.popup) mounted.delete(this.view);
    this.popup.dispose();
  }

  /** Pressing a candidate never takes the caret out of the prompt. */
  private readonly pressed = (event: MouseEvent): void => {
    if (event.button === 0) event.preventDefault();
  };

  /** Focus that reaches the gallery returns to the prompt, which keeps the one caret. */
  private readonly focused = (): void => {
    if (!this.disposed) void captureFocusHandoff(this.view).restore();
  };

  /** Hand the session's models to the gallery only when they or the current one changed. */
  private project(): void {
    const session = this.view.state.field(completionSession);
    if (!session || session.result.surface !== 'models') return;
    if (this.shown === session.result && this.selected === session.selected) return;
    this.shown = session.result;
    this.selected = session.selected;
    this.popup.update(
      session.result.options.map((option) => option.item),
      session.selected,
    );
  }

  /** Accept the pointed candidate now, or as soon as a refreshing session is current again. */
  private activate(index: number): void {
    if (this.disposed) return;
    const session = this.view.state.field(completionSession);
    if (!session || session.result !== this.shown || !session.result.options[index]) return;
    if (acceptCompletionOption(this.view, session.result, index, 'pointer')) return;
    if (session.phase === 'refreshing')
      this.view.dispatch({
        effects: requestCompletionAcceptance.of({
          result: session.result,
          index,
          activation: 'pointer',
        }),
        annotations: Transaction.addToHistory.of(false),
      });
  }

  /** Bound the surface to the space left on the side CodeMirror chose. */
  private measure(): void {
    if (this.disposed || this.queued) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      read: () => {
        const session = this.view.state.field(completionSession);
        return session && this.space
          ? {
              session,
              height: readCompletionHeight(this.view, this.dom, this.space, session.caret),
            }
          : null;
      },
      write: (measured) => {
        this.queued = false;
        if (this.disposed || !measured) return;
        const current = (): boolean =>
          !this.disposed && this.view.state.field(completionSession) === measured.session;
        if (applyCompletionHeight(this.dom, measured.height, current) === 'changed') this.measure();
      },
    });
  }
}

/** Stable factory: CodeMirror keeps one gallery across selection changes of the same session. */
export function createModelCompletionTooltip(view: EditorView): TooltipView {
  return new ModelCompletionTooltip(view);
}
