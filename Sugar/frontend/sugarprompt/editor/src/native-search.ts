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
/** Mounted native search coordinates host lifetime, prepared paint and existing native focus/input authorities. */
import { requireValue } from '../../core/src/required.js';
import { EditorSelection, Transaction } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { SearchSession } from '../../services/src/search/session.js';
import {
  captureSearchContext,
  captureSearchNavigation,
  captureSearchResults,
  captureSearchRevealAdapter,
} from '../../services/src/search/capture.js';
import type {
  SearchContext,
  SearchHostIntent,
  SearchNavigation,
  SearchOutcome,
  SearchRequest,
  SearchResults,
  SearchRevealAdapter,
  SearchSnapshot,
} from '../../services/src/search/contracts.js';
import { editorFreshness } from './freshness.js';
import { nativeCompositionStarted } from './input-settlement.js';
import type { NativeSurface } from './native-surface.js';
import {
  installSearchMatches,
  prepareSearchMatches,
  searchMatches,
  searchSnapshot,
  type PreparedSearch,
} from './search-matches.js';
/** One finite owner admits continuations only while every native and host identity remains current. */
export class NativeSearch {
  private readonly session: SearchSession;
  private freshness: ReturnType<NativeSearch['currentFreshness']>;
  private composition: boolean;
  private adapter: SearchRevealAdapter | null = null;
  private generation = 0;
  private focusAbort: AbortController | undefined;
  private disposed = false;
  private readonly surface: NativeSurface;
  private readonly revision: () => number;
  /** Host callbacks observe recomputation/visibility; the existing surface owns focus readiness. */
  constructor(
    surface: NativeSurface,
    revision: () => number,
    refresh: (request: SearchRequest) => void,
    intent: (intent: SearchHostIntent) => void,
  ) {
    this.surface = surface;
    this.revision = revision;
    this.freshness = this.currentFreshness();
    this.composition = nativeCompositionStarted(surface.view);
    this.session = new SearchSession(refresh, intent);
  }
  /** Read the existing native freshness owner without constructing source text. */
  private currentFreshness() {
    return this.surface.view.state.field(editorFreshness);
  }
  /** Generation protects external callbacks and avoids accessing a disposed native surface. */
  private current(generation: number): boolean {
    return !this.disposed && generation === this.generation;
  }
  /** Each search intent cancels only its own pending canonical focus before reentrant cancellation can issue newer work. */
  private advance(): number {
    const generation = ++this.generation,
      previous = this.focusAbort;
    this.focusAbort = undefined;
    previous?.abort();
    return generation;
  }
  /** The canonical native owner controls readiness; search retains only its own cancellation signal until settlement. */
  private async focus(generation: number): Promise<boolean> {
    if (!this.current(generation)) return false;
    const controller = new AbortController();
    this.focusAbort = controller;
    try {
      return await this.surface.focus('preserve-viewport', controller.signal);
    } finally {
      if (this.focusAbort === controller) this.focusAbort = undefined;
    }
  }
  /** A prepared publication/navigation also requires exact current source/context and query/result identity. */
  private admits(generation: number, prepared: PreparedSearch): boolean {
    return (
      this.current(generation) &&
      !nativeCompositionStarted(this.surface.view) &&
      this.currentFreshness() === prepared.freshness &&
      this.session.accepts(prepared.results.request)
    );
  }
  /** Composition observation consumes the actual input facet through the existing publication boundary. */
  observe(): void {
    if (this.disposed) return;
    const freshness = this.currentFreshness(),
      composition = nativeCompositionStarted(this.surface.view);
    const changed = freshness !== this.freshness,
      ended = this.composition && !composition;
    if (!changed && composition === this.composition) return;
    this.freshness = freshness;
    this.composition = composition;
    const generation = this.advance();
    if (!this.current(generation)) return;
    this.session.cancel();
    if (!this.current(generation)) return;
    if (changed || ended) this.session.retire(this.revision());
  }
  /** Context replacement validates before retiring paint, then checks every synchronous cancellation/dispatch boundary. */
  private changeContext(context: SearchContext | null): {
    readonly generation: number;
    readonly accepted: boolean;
    readonly request: SearchRequest | null;
  } {
    const generation = this.advance();
    const captured = captureSearchContext(context, () => this.current(generation));
    if (captured.status === 'stale')
      return { generation, accepted: false, request: this.session.current() };
    this.surface.retireHandoff('preserve-input');
    if (!this.current(generation))
      return { generation, accepted: false, request: this.session.current() };
    this.session.cancel();
    if (!this.current(generation))
      return { generation, accepted: false, request: this.session.current() };
    this.surface.view.dispatch({
      effects: installSearchMatches.of(null),
      annotations: Transaction.addToHistory.of(false),
    });
    if (!this.current(generation))
      return { generation, accepted: false, request: this.session.current() };
    const revision = this.revision();
    if (!this.current(generation))
      return { generation, accepted: false, request: this.session.current() };
    this.session.setContext(captured.value, revision);
    return {
      generation,
      accepted: this.current(generation),
      request: this.session.current(),
    };
  }
  /** Newer query/field intent from synchronous host callbacks wins over the issuing caller. */
  setContext(context: SearchContext | null): SearchRequest | null {
    return this.changeContext(context).request;
  }
  /** Replace the host adapter only after cancellation callbacks have had the chance to supersede this intent. */
  setAdapter(adapter: SearchRevealAdapter | null): void {
    const generation = this.advance();
    const captured = captureSearchRevealAdapter(adapter, () => this.current(generation));
    if (captured.status === 'stale') return;
    this.surface.retireHandoff('preserve-input');
    if (!this.current(generation)) return;
    this.session.cancel();
    if (this.current(generation)) this.adapter = captured.value;
  }
  /** Publish increasing current host results; reentrant cancellation cannot lower or clear newer output. */
  publish(input: SearchResults): SearchOutcome {
    if (this.disposed) return { status: 'stale' };
    const generation = this.advance();
    const captured = captureSearchResults(input, () => this.current(generation));
    if (captured.status === 'stale') return { status: 'stale' };
    const results = captured.value;
    if (
      this.disposed ||
      !this.session.accepts(results.request) ||
      nativeCompositionStarted(this.surface.view)
    )
      return { status: 'stale' };
    const current = this.surface.view.state.field(searchMatches)?.results;
    if (current && results.resultRevision <= current.resultRevision) return { status: 'stale' };
    const prepared = prepareSearchMatches(this.surface.view.state, results);
    this.surface.retireHandoff('preserve-input');
    if (!this.admits(generation, prepared)) return { status: 'stale' };
    this.session.cancel();
    if (!this.admits(generation, prepared)) return { status: 'stale' };
    const latest = this.surface.view.state.field(searchMatches)?.results;
    if (latest && results.resultRevision <= latest.resultRevision) return { status: 'stale' };
    this.surface.view.dispatch({
      effects: installSearchMatches.of(prepared),
      annotations: Transaction.addToHistory.of(false),
    });
    return {
      status:
        this.admits(generation, prepared) &&
        this.surface.view.state.field(searchMatches) === prepared
          ? 'applied'
          : 'stale',
    };
  }
  /** Navigation rechecks actual native composition and identity after host reveal, dispatch and measured focus. */
  async navigate(input: SearchNavigation): Promise<SearchOutcome> {
    if (this.disposed) return { status: 'cancelled' };
    if (nativeCompositionStarted(this.surface.view)) return { status: 'unavailable' };
    const generation = this.advance();
    const captured = captureSearchNavigation(input, () => this.current(generation));
    if (captured.status === 'stale') return { status: 'stale' };
    const intent = captured.value;
    const prepared = this.surface.view.state.field(searchMatches),
      index = prepared?.byId.get(intent.matchId) ?? -1;
    if (
      !prepared ||
      index < 0 ||
      !this.session.accepts(intent.request) ||
      prepared.results.request !== intent.request ||
      prepared.results.resultRevision !== intent.resultRevision
    )
      return { status: 'stale' };
    this.surface.retireHandoff('preserve-input');
    if (
      !this.admits(generation, prepared) ||
      this.surface.view.state.field(searchMatches) !== prepared
    )
      return { status: 'stale' };
    const reveal = await this.session.reveal(intent.request, intent.matchId, this.adapter);
    if (reveal !== 'ready') return { status: reveal };
    if (
      !this.admits(generation, prepared) ||
      this.surface.view.state.field(searchMatches) !== prepared
    )
      return { status: 'stale' };
    const range = requireValue(prepared.ranges[index], 'native-search'),
      selected = {
        ...prepared,
        results: { ...prepared.results, activeIndex: index },
      };
    this.surface.retireHandoff('preserve-input');
    if (
      !this.admits(generation, prepared) ||
      this.surface.view.state.field(searchMatches) !== prepared
    )
      return { status: 'stale' };
    this.surface.view.dispatch({
      ...(intent.select ? { selection: EditorSelection.range(range.from, range.to) } : {}),
      effects: [
        installSearchMatches.of(selected),
        ...(intent.scroll
          ? [EditorView.scrollIntoView(EditorSelection.range(range.from, range.to))]
          : []),
      ],
      annotations: Transaction.addToHistory.of(false),
    });
    if (
      !this.admits(generation, selected) ||
      this.surface.view.state.field(searchMatches) !== selected
    )
      return { status: 'stale' };
    if (!intent.focus) return { status: 'applied' };
    const focused = await this.focus(generation);
    return {
      status:
        !this.admits(generation, selected) ||
        this.surface.view.state.field(searchMatches) !== selected
          ? 'stale'
          : focused
            ? 'applied'
            : 'unavailable',
    };
  }
  /** Clear follows the same query admission boundary and never focuses over newer or composing intent. */
  async clear(focus: boolean): Promise<SearchOutcome> {
    if (typeof focus !== 'boolean') throw new TypeError('Invalid search clear focus intent');
    const cleared = this.changeContext(null);
    if (!cleared.accepted || cleared.request !== null) return { status: 'stale' };
    if (!focus) return { status: 'applied' };
    if (nativeCompositionStarted(this.surface.view)) return { status: 'unavailable' };
    const freshness = this.currentFreshness(),
      focused = await this.focus(cleared.generation);
    return {
      status:
        !this.current(cleared.generation) ||
        nativeCompositionStarted(this.surface.view) ||
        this.currentFreshness() !== freshness ||
        this.session.current() !== null
          ? 'stale'
          : focused
            ? 'applied'
            : 'unavailable',
    };
  }
  /** Observe source-free current identity without retaining another result owner. */
  snapshot(): SearchSnapshot {
    return searchSnapshot(this.surface.view.state, this.session.current());
  }
  /** Close native admission and focus readiness before external abort callbacks can attempt revival. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.adapter = null;
    try {
      this.advance();
    } finally {
      this.session.dispose();
    }
  }
}
