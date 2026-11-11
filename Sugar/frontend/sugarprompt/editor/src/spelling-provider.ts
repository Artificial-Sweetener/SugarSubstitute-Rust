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
/** One mounted native lifetime observes prepared scopes and delegates finite dictionary work to the shared host session. */
import { requireValue } from '../../core/src/required.js';
import { Facet, Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { TagLexicon } from '../../core/src/index.js';
import { LatestRequest } from '../../services/src/requests/latest.js';
import { prepareSpellingCandidates } from '../../services/src/diagnostics/spelling-preparation.js';
import type {
  PreparedSpellingCandidates,
  PreparedSpellingSuggestions,
  SpellingOutcome,
  SpellingIgnoreResult,
} from '../../services/src/diagnostics/spelling-contracts.js';
import type { SpellcheckSession } from '../../services/src/diagnostics/spelling-session.js';
import { featureEnabled } from '../../services/src/features/profile.js';
import { featureProfile } from './features.js';
import { semanticStructures } from './structure.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import {
  spellingSession,
  currentSpellingDiagnostic,
  prepareNativeSpellingDiagnostics,
  installSpellingDiagnostics,
  retireSpellingPaint,
  type SpellingDiagnosticValue,
} from './spelling-diagnostics.js';
/** The mounted facade delegates to the existing prepared lexicon getter without a second catalog owner. */
export const spellingLexicon = Facet.define<
  () => TagLexicon | undefined,
  () => TagLexicon | undefined
>({ combine: (values) => values[0] ?? (() => undefined) });
/** Unexpected causes cross one source-free host boundary; expected absence stays typed data. */
export const spellingErrors = Facet.define<
  (cause: unknown) => void,
  ((cause: unknown) => void) | null
>({ combine: (values) => values[0] ?? null });
const issuedSuggestions = new WeakSet<PreparedSpellingSuggestions>();
/** Only actual current results returned through the mounted suggestion boundary prepare corrective leaves. */
export function isNativeSpellingSuggestions(value: PreparedSpellingSuggestions): boolean {
  return issuedSuggestions.has(value);
}
/** Source/context/session/lexicon identity owns cancellation; session revision separately owns diagnostic publication. */
interface Context {
  readonly freshness: EditorFreshness;
  readonly session: SpellcheckSession;
  readonly lexicon: TagLexicon | null;
}
/** One view owns preparation, request cancellation and subscription release, while the host retains session ignores. */
export class SpellcheckProvider {
  private readonly checks = new LatestRequest();
  private readonly suggestions = new LatestRequest();
  private readonly commands = new LatestRequest();
  private release: (() => void) | null = null;
  private context: Context | null = null;
  private prepared: PreparedSpellingCandidates | undefined;
  private revision = -1;
  private queued = false;
  private disposed = false;
  private composing = false;
  private busy = false;
  private readonly view: EditorView;
  /** Initial current scopes qualify through the same off-input path as subsequent semantic publications. */
  constructor(view: EditorView) {
    this.view = view;
    this.queue();
  }
  /** Actual native publications are observed without adding a semantic preparation hook. */
  update(_update: ViewUpdate): void {
    this.observeContext();
    this.queue();
  }
  /** Preedit retires every mounted operation immediately. */
  compositionStart(): void {
    this.composing = true;
    this.cancel();
    this.view.dispatch({
      effects: retireSpellingPaint.of(null),
      annotations: Transaction.addToHistory.of(false),
    });
  }
  /** Resume after the browser's committed native state is available. */
  compositionEnd(): void {
    this.composing = false;
    this.queue();
  }
  /** Menu dismissal owns only its suggestion lane; accepted ignore/add may finish through their own revision. */
  cancelSuggestions(): void {
    this.suggestions.cancel();
  }
  /** Capture current host dependencies once; replacement retires accepted operations as well as derived results. */
  private observeContext(): Context | null {
    const state = this.view.state,
      session = state.facet(spellingSession),
      lexicon = state.facet(spellingLexicon)() ?? null;
    const eligible =
      !this.disposed &&
      !this.composing &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      featureEnabled(state.facet(featureProfile), 'spellcheck') &&
      session !== null;
    const old = this.context;
    if (
      !eligible ||
      !old ||
      old.session !== session ||
      old.lexicon !== lexicon ||
      old.freshness !== state.field(editorFreshness)
    ) {
      this.cancel();
      this.release?.();
      this.release = null;
      this.context = eligible
        ? {
            freshness: state.field(editorFreshness),
            session: requireValue(session, 'spelling-provider'),
            lexicon,
          }
        : null;
      this.revision = -1;
      if (this.context)
        this.release = this.context.session.subscribe(() => {
          this.checks.cancel();
          this.suggestions.cancel();
          this.revision = -1;
          this.queue();
        });
    }
    return this.context;
  }
  /** One queued native preparation never runs in a key/composition handler. */
  queue(): void {
    if (this.disposed || this.queued) return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      if (this.disposed) return;
      void this.prepare().catch((cause: unknown) => this.report(cause));
    });
  }
  /** Real service/Core preparations are reused only under their complete actual dependency identities. */
  async prepare(): Promise<void> {
    const context = this.observeContext(),
      native = this.view.state.field(semanticStructures).native;
    if (!context || !native) return;
    if (
      this.prepared?.scopes === native.scopes &&
      this.prepared.lexicon === context.lexicon &&
      this.revision === context.session.revision
    )
      return;
    const revision = context.session.revision;
    this.revision = revision;
    const prepared = prepareSpellingCandidates(native.scopes, context.lexicon, this.prepared);
    this.prepared = prepared;
    const pending = prepareNativeSpellingDiagnostics(
      this.view.state,
      context.session,
      null,
      'pending',
      null,
      prepared,
    );
    if (pending && this.live(context))
      this.view.dispatch({
        effects: installSpellingDiagnostics.of(pending),
        annotations: Transaction.addToHistory.of(false),
      });
    if (
      !this.live(context) ||
      revision !== context.session.revision ||
      this.view.state.field(semanticStructures).native?.scopes !== prepared.scopes
    )
      return;
    const outcome = await this.checks
      .run((signal) => context.session.check(prepared, signal))
      .catch((cause: unknown) => {
        if (this.live(context) && revision === context.session.revision) {
          const failed = prepareNativeSpellingDiagnostics(
            this.view.state,
            context.session,
            null,
            'failed',
          );
          if (failed)
            try {
              this.view.dispatch({
                effects: installSpellingDiagnostics.of(failed),
                annotations: Transaction.addToHistory.of(false),
              });
            } catch (publicationCause) {
              throw new AggregateError(
                [cause, publicationCause],
                'spelling-check-publication-failed',
              );
            }
        }
        throw cause;
      });
    if (
      outcome.status !== 'ready' ||
      !this.live(context) ||
      revision !== context.session.revision ||
      this.view.state.field(semanticStructures).native?.scopes !== prepared.scopes
    )
      return;
    const result = outcome.value;
    if (result.status !== 'ready' && result.status !== 'unavailable') return;
    const value = prepareNativeSpellingDiagnostics(
      this.view.state,
      context.session,
      result.status === 'ready' ? result.value : null,
      result.status,
      result.status === 'unavailable' ? result.code : null,
    );
    if (value && this.live(context))
      this.view.dispatch({
        effects: installSpellingDiagnostics.of(value),
        annotations: Transaction.addToHistory.of(false),
      });
  }
  /** Native context identity is checked again after every external result or synchronous observer. */
  private live(context: Context): boolean {
    return (
      !this.disposed &&
      !this.composing &&
      !this.view.composing &&
      !this.view.compositionStarted &&
      this.context === context &&
      this.view.state.field(editorFreshness) === context.freshness &&
      this.view.state.facet(spellingSession) === context.session &&
      this.view.state.facet(spellingLexicon)() === (context.lexicon ?? undefined) &&
      featureEnabled(this.view.state.facet(featureProfile), 'spellcheck')
    );
  }
  /** Lazy suggestions belong only to the current indexed issue and one mounted menu lane. */
  async suggest(
    value: SpellingDiagnosticValue,
  ): Promise<SpellingOutcome<PreparedSpellingSuggestions>> {
    const context = this.observeContext();
    if (!context || !currentSpellingDiagnostic(this.view.state, value))
      return { status: 'rejected' };
    const result = await this.suggestions.run((signal) =>
      value.session.suggestions(value.snapshot, value.issue, signal),
    );
    if (
      result.status !== 'ready' ||
      !this.live(context) ||
      !currentSpellingDiagnostic(this.view.state, value)
    )
      return { status: this.disposed ? 'disposed' : 'cancelled' };
    if (result.value.status === 'ready') issuedSuggestions.add(result.value.value);
    return result.value;
  }
  /** Accepted dictionary/session commands are exclusive; their own revision and source-neutral menu close do not cancel them. */
  async operate(value: SpellingDiagnosticValue, action: 'ignore' | 'add'): Promise<void> {
    const context = this.observeContext();
    if (this.busy || !context || !currentSpellingDiagnostic(this.view.state, value)) return;
    this.busy = true;
    try {
      await this.commands.run<SpellingOutcome<boolean | SpellingIgnoreResult>>((signal) =>
        action === 'ignore'
          ? value.session.ignore(value.snapshot, value.issue, signal)
          : value.session.add(value.snapshot, value.issue, signal),
      );
    } finally {
      this.busy = false;
    }
  }
  /** Unexpected current causes are observable; never convert them into an empty ready result. */
  report(cause: unknown): void {
    const observer = this.view.state.facet(spellingErrors);
    if (observer) observer(cause);
    else throw cause;
  }
  /** Retirement settles every owned operation but does not dispose the shared host dictionary. */
  private cancel(): void {
    this.checks.cancel();
    this.suggestions.cancel();
    this.commands.cancel();
  }
  /** Mounted teardown releases listeners, view-derived preparations and all external lanes. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.release?.();
    this.release = null;
    this.cancel();
    this.checks.dispose();
    this.suggestions.dispose();
    this.commands.dispose();
    this.context = null;
    this.prepared = undefined;
  }
}
/** Native lifecycle observes composition/settlement through supported CodeMirror APIs. */
export const spellingProvider = ViewPlugin.fromClass(SpellcheckProvider, {
  eventObservers: {
    keyup() {
      this.queue();
    },
    blur() {
      this.queue();
    },
    compositionstart() {
      this.compositionStart();
    },
    compositionend() {
      this.compositionEnd();
    },
  },
});
