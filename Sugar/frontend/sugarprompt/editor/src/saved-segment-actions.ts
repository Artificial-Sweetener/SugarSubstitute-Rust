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
/** Native modal presentation owns label/scope controls and never adopts the editor document or history. */
import { Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { SavedSegmentSession } from '../../services/src/saved-segments/session.js';
import {
  savedSegmentHost,
  publishSavedSegments,
  currentSavedSegmentMenu,
  type PreparedSavedSegmentMenu,
} from './saved-segment-menu.js';
import { editorFreshness } from './freshness.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { strip } from '../../core/src/prompt/text.js';
interface SaveDialog {
  readonly dom: HTMLDialogElement;
  readonly release: () => void;
}
/** One view owns finite storage refresh/save work and one native literal modal. */
export class SavedSegmentActions {
  private readonly view: EditorView;
  private session: SavedSegmentSession | null = null;
  private dialog: SaveDialog | null = null;
  private disposed = false;
  private generation = 0;
  /** Defer preparation until the native plugin is mounted. */
  constructor(view: EditorView) {
    this.view = view;
    queueMicrotask(() => this.configure());
  }
  /** Host replacement disposes its lane; source/selection/composition retirement dismisses captured source intent. */
  update(update: ViewUpdate): void {
    if (update.startState.facet(savedSegmentHost) !== update.state.facet(savedSegmentHost)) {
      try {
        this.retireSession();
      } finally {
        queueMicrotask(() => this.configure());
      }
    } else if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      !update.startState.selection.eq(update.state.selection) ||
      this.view.composing ||
      this.view.compositionStarted
    )
      this.cancel();
  }
  /** Prepare a fresh catalog only for the current complete injected host. */
  private configure(): void {
    if (this.disposed || this.session) return;
    const host = this.view.state.facet(savedSegmentHost);
    if (!host) return;
    this.session = new SavedSegmentSession(host.adapter, host.context);
    void this.refresh();
  }
  /** Storage traversal remains outside input and publishes only to its finite current generation. */
  private async refresh(): Promise<void> {
    const host = this.view.state.facet(savedSegmentHost),
      session = this.session,
      generation = ++this.generation;
    if (!host || !session || this.disposed) return;
    this.view.dispatch({
      effects: publishSavedSegments.of({
        host,
        status: 'pending',
        catalog: null,
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    try {
      const result = await session.read();
      if (
        this.disposed ||
        generation !== this.generation ||
        host !== this.view.state.facet(savedSegmentHost)
      )
        return;
      if (result.status === 'cancelled' || result.status === 'disposed') return;
      this.view.dispatch({
        effects: publishSavedSegments.of({
          host,
          status: result.status === 'ready' ? 'ready' : 'unavailable',
          catalog: result.status === 'ready' ? result.catalog : null,
        }),
        annotations: Transaction.addToHistory.of(false),
      });
    } catch (cause) {
      if (!this.disposed && generation === this.generation) {
        this.view.dispatch({
          effects: publishSavedSegments.of({
            host,
            status: 'unavailable',
            catalog: null,
          }),
          annotations: Transaction.addToHistory.of(false),
        });
        host.report(cause);
      }
    }
  }
  /** Open native label/scope inputs for one already admitted menu capture. */
  save(menu: PreparedSavedSegmentMenu, handoff: FocusHandoff): void {
    try {
      this.cancel();
    } catch (cause) {
      try {
        handoff.cancel();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'saved-segment-dialog-supersession-failed');
      }
      throw cause;
    }
    if (
      this.disposed ||
      !currentSavedSegmentMenu(this.view.state, menu) ||
      !menu.saveLeaf ||
      !menu.publication?.catalog ||
      !this.session ||
      this.view.state.readOnly
    ) {
      handoff.cancel();
      return;
    }
    try {
      this.open(menu, handoff);
    } catch (cause) {
      const causes: unknown[] = [cause];
      try {
        if (this.dialog) this.cancel();
        else handoff.cancel();
      } catch (cleanup) {
        causes.push(cleanup);
      }
      if (causes.length === 1) throw cause;
      throw new AggregateError(causes, 'saved-segment-dialog-construction-failed');
    }
  }
  /** Build one literal modal only after native capture admission; construction failures return through the same finite retirement path. */
  private open(menu: PreparedSavedSegmentMenu, handoff: FocusHandoff): void {
    if (!menu.publication?.catalog || !this.session)
      throw new Error('saved-segment-dialog-admission-lost');
    const document = this.view.dom.ownerDocument,
      dom = document.createElement('dialog'),
      form = document.createElement('form'),
      heading = document.createElement('h2'),
      label = document.createElement('label'),
      input = document.createElement('input'),
      scopeLabel = document.createElement('label'),
      select = document.createElement('select'),
      status = document.createElement('p'),
      submit = document.createElement('button'),
      cancel = document.createElement('button');
    dom.className = 'sp-save-segment-dialog';
    dom.setAttribute('aria-label', 'Save selected segment');
    heading.textContent = 'Save selected segment';
    label.textContent = 'Label ';
    input.required = true;
    input.name = 'label';
    input.maxLength = 512;
    label.append(input);
    scopeLabel.textContent = 'Scope ';
    select.name = 'scope';
    scopeLabel.append(select);
    const scopes = menu.publication.catalog.context.scopes;
    scopes.forEach((scope, index) => {
      const option = document.createElement('option');
      option.value = String(index);
      option.textContent = scope.label;
      select.append(option);
    });
    submit.type = 'submit';
    submit.textContent = 'Save';
    cancel.type = 'button';
    cancel.textContent = 'Cancel';
    status.setAttribute('role', 'status');
    form.append(heading, label, scopeLabel, status, submit, cancel);
    dom.append(form);
    const freshness = this.view.state.field(editorFreshness),
      selection = this.view.state.selection,
      host = menu.publication.host,
      session = this.session;
    /** Modal input may never restore focus after a newer external destination or native intent. */
    let retiring = false;
    const finish = (restore: boolean, primary?: unknown): void => {
      if (this.dialog?.dom !== dom) return;
      if (retiring) return;
      retiring = true;
      const causes: unknown[] = primary === undefined ? [] : [primary];
      const steps = [
        () => session.cancel(),
        () => dom.removeEventListener('cancel', dismiss),
        () => cancel.removeEventListener('click', dismiss),
        () => form.removeEventListener('submit', accept),
        () => {
          if (dom.open) dom.close();
        },
        () => dom.remove(),
        () => {
          if (dom.isConnected) dom.parentNode?.removeChild(dom);
        },
        () => {
          dom.textContent = '';
        },
        () => {
          if (restore)
            void handoff.restore('preserve-viewport').catch((cause: unknown) => host.report(cause));
          else handoff.cancel();
        },
      ];
      for (const step of steps) {
        try {
          step();
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (this.dialog?.dom === dom && !dom.isConnected) this.dialog = null;
      retiring = false;
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'saved-segment-dialog-retirement-failed');
    };
    /** Escape and explicit cancellation retire persistence before focus handoff. */
    const dismiss = (event: Event): void => {
      event.preventDefault();
      finish(true);
    };
    /** Persist only the unchanged capture and literal admitted scope; failures remain visible without losing label input. */
    const accept = (event: SubmitEvent): void => {
      event.preventDefault();
      if (submit.disabled || !strip(input.value)) return;
      if (
        freshness !== this.view.state.field(editorFreshness) ||
        !selection.eq(this.view.state.selection) ||
        !currentSavedSegmentMenu(this.view.state, menu)
      ) {
        finish(false);
        return;
      }
      const scope = scopes[Number(select.value)];
      if (!scope) return;
      submit.disabled = true;
      input.disabled = true;
      select.disabled = true;
      status.textContent = 'Saving…';
      void session
        .save(input.value, menu.selectedText, scope)
        .then((result) => {
          if (this.disposed || this.dialog?.dom !== dom) return;
          if (result.status === 'saved') {
            finish(true);
            void this.refresh();
          } else if (result.status === 'cancelled' || result.status === 'disposed') finish(false);
          else {
            submit.disabled = false;
            input.disabled = false;
            select.disabled = false;
            status.textContent = 'Saved segment storage is unavailable.';
          }
        })
        .catch((cause: unknown) => {
          if (!this.disposed && this.dialog?.dom === dom) {
            submit.disabled = false;
            input.disabled = false;
            select.disabled = false;
            status.textContent = 'Saving failed.';
          }
          host.report(cause);
        });
    };
    dom.addEventListener('cancel', dismiss);
    cancel.addEventListener('click', dismiss);
    form.addEventListener('submit', accept);
    this.dialog = { dom, release: () => finish(false) };
    document.body.append(dom);
    dom.showModal();
    input.focus();
  }
  /** Release source-bearing modal nodes and settle persistence on supersession. */
  private cancel(): void {
    this.dialog?.release();
  }
  /** Teardown consumes all resources before the native editor disappears. */
  destroy(): void {
    this.disposed = true;
    this.generation++;
    this.retireSession();
  }
  /** Host replacement and destruction attempt modal, persistence and identity retirement even when one release fails. */
  private retireSession(): void {
    const causes: unknown[] = [];
    try {
      this.cancel();
    } catch (cause) {
      causes.push(cause);
    }
    try {
      this.session?.dispose();
    } catch (cause) {
      causes.push(cause);
    }
    this.session = null;
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'saved-segment-owner-retirement-failed');
  }
}
/** Native state assembly installs one finite saved-segment owner per real editor. */
export const savedSegmentActions = ViewPlugin.fromClass(SavedSegmentActions);
