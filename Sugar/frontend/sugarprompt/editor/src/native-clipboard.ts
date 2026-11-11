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
/** One native lifetime binds asynchronous clipboard work to CodeMirror source, selection, permission and focus. */
import { requireValue } from '../../core/src/required.js';
import { Facet, type Extension } from '@codemirror/state';
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view';
import type { TagLexicon } from '../../core/src/index.js';
import { ClipboardSession } from '../../services/src/clipboard/session.js';
import type { ClipboardTransfer } from '../../services/src/clipboard/contracts.js';
import { BooruPasteImport } from './booru-paste-import.js';
import { booruPostLookup } from './booru-paste-host.js';
import { browserClipboard } from './browser-clipboard.js';
import { clipboardCut, clipboardPaste, clipboardSelection } from './clipboard.js';
import { editorFreshness } from './freshness.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
/** Clipboard commands share native event source rules; the browser remains the system transfer boundary. */
export type ClipboardAction = 'copy' | 'cut' | 'paste';
/** Source-free command outcomes distinguish native availability, permission and current target retirement. */
export interface ClipboardCommandOutcome {
  readonly status:
    | 'completed'
    | 'read-only'
    | 'composing'
    | 'no-selection'
    | 'empty'
    | 'denied'
    | 'unavailable'
    | 'cancelled'
    | 'disposed'
    | 'stale';
}
/** The host observes expected outcomes and reports current unexpected failures without logging clipboard payloads. */
export interface ClipboardObserver {
  /** Publish current source-free completion or an expected availability/lifetime outcome. */
  observe(outcome: ClipboardCommandOutcome): void;
  /** Retain a current unexpected interaction cause at the mounted host reporting boundary. */
  error(cause: unknown): void;
}
/** Native state supplies its existing current lexicon observation and optional finite host reporting boundary. */
interface ClipboardConfiguration {
  readonly lexicon: () => TagLexicon | undefined;
  readonly observer: ClipboardObserver | undefined;
}
const clipboardConfiguration = Facet.define<ClipboardConfiguration, ClipboardConfiguration>({
  combine: (values) =>
    values[0] ?? {
      lexicon: () => undefined,
      observer: undefined,
    },
});
/** The mounted plugin owns one gateway and cancellation lane; no editable source or parallel selection is stored here. */
export class NativeClipboard {
  private readonly session: ClipboardSession;
  private readonly imports: BooruPasteImport;
  private readonly view: EditorView;
  readonly readable: boolean;
  readonly writable: boolean;
  private disposed = false;
  private operation = 0;
  /** Capture this document's browser boundary once, outside typing and menu preparation. */
  constructor(view: EditorView) {
    this.view = view;
    const owner = view.dom.ownerDocument.defaultView;
    if (!owner) throw new Error('Native clipboard window unavailable');
    const gateway = browserClipboard(owner);
    this.readable = gateway.readable;
    this.writable = gateway.writable;
    this.session = new ClipboardSession(gateway);
    this.imports = new BooruPasteImport(
      view,
      () => view.state.facet(booruPostLookup) ?? undefined,
      () => view.state.facet(clipboardConfiguration).lexicon(),
    );
  }
  /** Source/context/selection, composition and external focus retirement settle pending work before it can mutate a new target. */
  update(update: Pick<ViewUpdate, 'startState' | 'state' | 'transactions' | 'focusChanged'>): void {
    this.imports.update(update);
    if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      !update.startState.selection.eq(update.state.selection) ||
      (update.focusChanged && !this.view.hasFocus) ||
      this.view.composing ||
      this.view.compositionStarted
    )
      this.session.cancel();
  }
  /** Native keyboard clipboard events supersede pending menu/API work even when they leave source unchanged. */
  cancel(): void {
    this.operation++;
    this.session.cancel();
    this.imports.cancel();
  }
  /** Execute only the deliberate current command; physical native writes cannot be rolled back after browser acceptance. */
  async execute(
    action: ClipboardAction,
    nativeTransfer?: ClipboardTransfer,
  ): Promise<ClipboardCommandOutcome> {
    let outcome: ClipboardCommandOutcome;
    if (this.disposed) return { status: 'disposed' };
    const operation = ++this.operation;
    this.session.cancel();
    this.imports.cancel();
    if (this.view.composing || this.view.compositionStarted) outcome = { status: 'composing' };
    else if (action !== 'copy' && this.view.state.readOnly) outcome = { status: 'read-only' };
    else {
      const state = this.view.state;
      const freshness = state.field(editorFreshness),
        selection = state.selection;
      const selected = clipboardSelection(state);
      if (action !== 'paste' && !selected) outcome = { status: 'no-selection' };
      else {
        const result =
          action === 'paste'
            ? await this.session.read(nativeTransfer)
            : await this.session.write(requireValue(selected, 'native-clipboard').text);
        if (result.status !== 'read' && result.status !== 'written') outcome = result;
        else if (
          this.disposed ||
          operation !== this.operation ||
          freshness !== this.view.state.field(editorFreshness) ||
          !selection.eq(this.view.state.selection) ||
          this.view.composing ||
          this.view.compositionStarted
        )
          outcome = { status: 'stale' };
        else {
          if (result.status === 'read') {
            const imported =
              result.value.kind === 'plain' ? this.imports.start(result.value.text) : null;
            if (imported) {
              await imported;
              outcome = { status: 'completed' };
            } else {
              const insertion = clipboardPaste(
                this.view.state,
                result.value,
                this.view.state.facet(clipboardConfiguration).lexicon(),
              );
              outcome = {
                status: !insertion
                  ? 'empty'
                  : dispatchNativeIngestion(this.view, insertion)
                    ? 'completed'
                    : 'stale',
              };
            }
          } else {
            const deletion = action === 'cut' ? clipboardCut(this.view.state) : null;
            if (deletion) this.view.dispatch(deletion);
            outcome = { status: 'completed' };
          }
        }
      }
    }
    if (!this.disposed && operation === this.operation)
      this.view.state.facet(clipboardConfiguration).observer?.observe(outcome);
    return outcome;
  }
  /** Native blur retires imports immediately; teardown, refocus and newer commands fence deferred clipboard cancellation. */
  blur(): void {
    const operation = this.operation;
    this.retireImport();
    queueMicrotask(() => {
      if (!this.disposed && operation === this.operation && !this.view.hasFocus)
        this.session.cancel();
    });
  }
  /** Key and pointer intent retire only import continuation, preserving ordinary clipboard request contracts. */
  retireImport(): void {
    this.imports.cancel();
  }
  /** Native plain paste publishes its literal before any provider task and uses this same mounted error boundary. */
  fromPasteImport(text: string): boolean {
    const operation = this.operation;
    try {
      const request = this.imports.start(text);
      if (!request) return false;
      void request.catch((cause: unknown) => {
        if (operation === this.operation) this.reportFailure(cause);
      });
      return true;
    } catch (cause) {
      this.reportFailure(cause);
      return true;
    }
  }
  /** Publish the source already accepted by a native copy/cut into the asynchronous API's private system format; source and native cut remain synchronous. */
  async copiedSource(text: string): Promise<void> {
    if (this.disposed || !this.writable) return;
    const operation = this.operation;
    try {
      const result = await this.session.write(text);
      if (!this.disposed && operation === this.operation)
        this.view.state.facet(clipboardConfiguration).observer?.observe({
          status: result.status === 'written' ? 'completed' : result.status,
        });
    } catch (cause) {
      if (operation === this.operation) this.reportFailure(cause);
    }
  }
  /** Native menus/events are error-reporting boundaries; public callers retain their rejected promise and original cause. */
  async fromInteraction(
    action: ClipboardAction,
    nativeTransfer?: ClipboardTransfer,
  ): Promise<void> {
    const operation = this.operation + 1;
    try {
      const outcome = await this.execute(action, nativeTransfer);
      if (!this.disposed && (outcome.status === 'denied' || outcome.status === 'unavailable'))
        this.view.dispatch({
          effects: EditorView.announce.of(`Clipboard ${outcome.status}.`),
        });
    } catch (cause) {
      if (operation === this.operation) this.reportFailure(cause);
    }
  }
  /** Unexpected current interaction failures retain their cause at the mounted host boundary. */
  private reportFailure(cause: unknown): void {
    if (this.disposed) return;
    const observer = this.view.state.facet(clipboardConfiguration).observer;
    if (observer) observer.error(cause);
    else throw cause;
  }
  /** Settle any ignored-abort system work before the native view releases its lifetime. */
  destroy(): void {
    this.disposed = true;
    this.session.dispose();
    this.imports.destroy();
  }
}
/** One stable plugin identity supports both the public source facade and native menu delegation. */
export const nativeClipboard = ViewPlugin.fromClass(NativeClipboard, {
  eventObservers: {
    /** Even navigation that leaves the native selection unchanged retires the pending paste. */
    keydown() {
      this.retireImport();
    },
    /** Pointer intent retires a request before any native selection transport. */
    pointerdown() {
      this.retireImport();
    },
    /** Physical focus loss retires requests independently of delayed state publication. */
    blur() {
      this.blur();
    },
  },
});
/** Mount clipboard orchestration alongside the existing ingestion lexicon and host reporting boundary. */
export function clipboardCommands(
  lexicon: () => TagLexicon | undefined,
  observer?: ClipboardObserver,
): Extension {
  return [clipboardConfiguration.of({ lexicon, observer }), nativeClipboard];
}
