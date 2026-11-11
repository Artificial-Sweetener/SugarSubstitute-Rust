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
/** Supported CodeMirror tooltip placement projects one native session into bounded, independently scrollable owned DOM. */
import { requireValue } from '../../core/src/required.js';
import { captureFocusHandoff } from './focus-handoff.js';
import { Transaction, type Extension } from '@codemirror/state';
import {
  EditorView,
  showTooltip,
  type TooltipView,
  type ViewUpdate,
  type Rect,
} from '@codemirror/view';
import {
  completionSession,
  type CompletionSession,
  requestCompletionAcceptance,
} from './completion-session.js';
import { completionWall, installCompletionWall } from './completion-wall-state.js';
import { prepareLoraWallLayout, loraWallVisibleRange } from './lora-wall-layout.js';
import {
  createCompletionOption,
  relabelCompletionRow,
  paintCompletionMedia,
  type CompletionOptionDOM,
} from './completion-option-dom.js';
import { acceptCompletionOption } from './prompt-completion-acceptance.js';
import type { PreparedCompletionResult } from './prompt-completion-model.js';
import { loraMedia } from './lora-media-state.js';
import { completionTheme } from './completion-theme.js';
import { CompletionScrollbar } from './completion-scrollbar.js';
import { CompletionTitleMarquee } from './completion-title-marquee.js';
import { readCompletionHeight, applyCompletionHeight } from './completion-space.js';
/** Geometry is read in CodeMirror measurement phases, compiled off input/paint, and published with candidate identity. */
class CompletionTooltip implements TooltipView {
  readonly dom = document.createElement('div');
  /** The adopted panel gap delegates both above/below placement to the supported native tooltip owner. */
  readonly offset = { x: 0, y: 6 } as const;
  /** This viewport reserves its own gap; native placement still owns the side and anchor. */
  readonly resize = false;
  private space: Rect | null = null;
  private readonly viewport = document.createElement('div');
  private readonly content = document.createElement('div');
  private readonly nodes = new Map<number, CompletionOptionDOM>();
  private readonly view: EditorView;
  private readonly observer: ResizeObserver;
  private readonly titles: CompletionTitleMarquee;
  private readonly scrollbar: CompletionScrollbar;
  private result: PreparedCompletionResult | null = null;
  private presentation: symbol | null = null;
  private disposed = false;
  private ensureSelected = true;
  private queued = false;
  private measurementRevision = 0;
  /** Own tooltip nodes and listeners; never create another input or a competing selection owner. */
  constructor(view: EditorView) {
    this.view = view;
    this.dom.className = 'cm-tooltip-autocomplete';
    this.viewport.className = 'sp-completion-viewport';
    this.viewport.setAttribute('role', 'listbox');
    this.viewport.setAttribute('aria-label', 'Completions');
    this.viewport.tabIndex = 0;
    this.content.className = 'sp-completion-content';
    this.scrollbar = new CompletionScrollbar(this.viewport, () => this.measure());
    let titles: CompletionTitleMarquee | null = null;
    let observer: ResizeObserver | null = null;
    try {
      this.viewport.append(this.scrollbar.dom, this.content);
      this.dom.append(this.viewport);
      this.titles = titles = new CompletionTitleMarquee(view, this.viewport, () => {
        const selected = view.state.field(completionSession)?.selected;
        return selected === undefined ? null : (this.nodes.get(selected)?.dom ?? null);
      });
      this.viewport.addEventListener('scroll', this.scrolled);
      this.viewport.addEventListener('mousedown', this.pressed);
      this.viewport.addEventListener('focus', this.focused);
      this.observer = observer = new ResizeObserver(() => {
        try {
          this.scrollbar.invalidate();
          this.measure();
          this.titles.refresh(true);
        } catch (error) {
          this.fail(error);
        }
      });
      this.project();
    } catch (error) {
      this.disposed = true;
      const causes: unknown[] = [error];
      for (const release of [
        () => this.scrollbar.destroy(),
        () => titles?.destroy(),
        () => observer?.disconnect(),
        () => this.viewport.removeEventListener('scroll', this.scrolled),
        () => this.viewport.removeEventListener('mousedown', this.pressed),
        () => this.viewport.removeEventListener('focus', this.focused),
        () => this.dom.remove(),
      ]) {
        try {
          release();
        } catch (cleanup) {
          causes.push(cleanup);
        }
      }
      if (causes.length === 1) throw error;
      throw new AggregateError(causes, 'Completion tooltip acquisition failed');
    }
  }
  /** Observe only this finite tooltip's actual local viewport. */
  mount(): void {
    try {
      this.observer.observe(this.viewport);
      this.measure();
    } catch (error) {
      this.fail(error);
    }
  }
  /** Native candidate/selection state remains authoritative; raster publications only repaint retained visible nodes. */
  update(update: ViewUpdate): void {
    if (this.disposed) return;
    try {
      const before = update.startState.field(completionSession),
        after = update.state.field(completionSession);
      if (before?.result !== after?.result || before?.selected !== after?.selected) {
        this.ensureSelected = true;
        if (before?.selected !== after?.selected) this.scrollbar.invalidate();
        this.project();
        this.measure();
      } else if (update.startState.field(completionWall) !== update.state.field(completionWall)) {
        this.project();
        if (this.ensureSelected) this.measure();
      } else if (update.startState.field(loraMedia) !== update.state.field(loraMedia))
        this.paintMedia();
    } catch (error) {
      this.fail(error);
    }
  }
  /** Retain only native available-space operands; actual caret and box geometry stay in READ. */
  positioned(space: Rect): void {
    this.space = { ...space };
    this.measure();
  }
  /** Teardown prevents queued geometry/media publication and releases every owned observation and node. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.measurementRevision++;
    this.result = null;
    this.nodes.clear();
    const causes: unknown[] = [];
    for (const release of [
      () => this.scrollbar.destroy(),
      () => this.titles.destroy(),
      () => this.observer.disconnect(),
      () => this.viewport.removeEventListener('scroll', this.scrolled),
      () => this.viewport.removeEventListener('mousedown', this.pressed),
      () => this.viewport.removeEventListener('focus', this.focused),
    ]) {
      try {
        release();
      } catch (error) {
        causes.push(error);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length > 1) throw new AggregateError(causes, 'Completion tooltip retirement failed');
  }
  /** Independent wall scrolling changes only derived viewport membership. */
  private readonly scrolled = (): void => {
    this.measure();
  };
  /** Keyboard/assistive entry delegates immediately to the native textbox and its active descendant, preserving one caret owner. */
  private readonly focused = (): void => {
    if (!this.disposed) void captureFocusHandoff(this.view).restore();
  };
  /** Pointer acceptance uses the actual rendered candidate and native freshness, retaining editable focus. */
  private readonly pressed = (event: MouseEvent): void => {
    if (event.button !== 0 || !(event.target instanceof Element)) return;
    const target = event.target.closest<HTMLElement>('[role=option]');
    if (!target || !this.content.contains(target)) return;
    const node = this.nodes.get(Number(target.dataset.index));
    const session = this.view.state.field(completionSession);
    event.preventDefault();
    if (node?.dom === target && session?.result === this.result) {
      const index = Number(target.dataset.index);
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
  };
  /** Coalesce native measurement; preparation never walks candidates in the keyboard or paint stack. */
  private measure(): void {
    if (this.disposed) return;
    this.measurementRevision++;
    if (this.queued) return;
    this.queued = true;
    try {
      this.view.requestMeasure({
        key: this,
        read: () => {
          try {
            const session = this.view.state.field(completionSession);
            const selected = session?.selected;
            const node = selected === undefined ? undefined : this.nodes.get(selected)?.dom;
            return {
              revision: this.measurementRevision,
              result: session?.result,
              presentation: session?.presentation,
              available:
                session && this.space
                  ? readCompletionHeight(this.view, this.dom, this.space, session.caret)
                  : undefined,
              scrollbar: this.scrollbar.read(),
              width: this.viewport.clientWidth,
              height: this.viewport.clientHeight,
              top: this.viewport.scrollTop,
              optionTop: node?.offsetTop,
              optionHeight: node?.offsetHeight,
            };
          } catch (error) {
            return this.fail(error);
          }
        },
        write: (measurement) => {
          try {
            this.queued = false;
            if (this.disposed) return;
            const session = this.view.state.field(completionSession);
            if (!session || session.result !== this.result) return;
            if (
              session.result !== measurement.result ||
              session.presentation !== measurement.presentation ||
              this.measurementRevision !== measurement.revision
            ) {
              this.measure();
              return;
            }
            const current = () =>
              !this.disposed &&
              this.measurementRevision === measurement.revision &&
              this.view.state.field(completionSession) === session;
            if (measurement.available !== undefined) {
              const height = applyCompletionHeight(this.dom, measurement.available, current);
              if (height !== 'unchanged') {
                if (height === 'changed') {
                  this.scrollbar.invalidate();
                  if (current()) this.measure();
                }
                return;
              }
            }
            const wall = this.view.state.field(completionWall);
            let top = measurement.top;
            const box =
              wall?.result === session.result ? wall.layout.boxes[session.selected] : undefined;
            const optionTop = box?.y ?? measurement.optionTop,
              optionHeight = box?.height ?? measurement.optionHeight;
            if (this.ensureSelected && optionTop !== undefined && optionHeight !== undefined) {
              if (optionTop < top) top = optionTop;
              else if (optionTop + optionHeight > top + measurement.height)
                top = optionTop + optionHeight - measurement.height;
              this.viewport.scrollTop = Math.max(0, top);
              if (!current()) return;
              top = this.viewport.scrollTop;
              if (!current()) return;
              this.ensureSelected = false;
            }
            if (!current()) return;
            this.scrollbar.project(measurement.scrollbar && { ...measurement.scrollbar, top });
            if (
              this.disposed ||
              this.measurementRevision !== measurement.revision ||
              this.view.state.field(completionSession) !== session
            )
              return;
            if (requireValue(session.result.options[0], 'completion-tooltip').kind === 'lora')
              queueMicrotask(() => {
                try {
                  this.prepareWall(
                    session.result,
                    session.presentation,
                    measurement.revision,
                    measurement.width,
                    measurement.height,
                    top,
                  );
                } catch (error) {
                  this.fail(error);
                }
              });
          } catch (error) {
            this.fail(error);
          }
        },
      });
    } catch (error) {
      this.fail(error);
    }
  }
  /** Unexpected native callbacks retire all owned resources and preserve the primary raw cause. */
  private fail(primary: unknown): never {
    try {
      this.destroy();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'Completion tooltip failed');
    }
    throw primary;
  }
  /** Compile geometry only after a real width change and prepare only visible/overscan thumbnail descriptors. */
  private prepareWall(
    result: PreparedCompletionResult,
    presentation: symbol,
    revision: number,
    width: number,
    height: number,
    top: number,
  ): void {
    if (
      this.disposed ||
      width <= 0 ||
      this.measurementRevision !== revision ||
      this.view.state.field(completionSession)?.presentation !== presentation ||
      this.view.state.field(completionSession)?.result !== result
    )
      return;
    const previous = this.view.state.field(completionWall);
    const layout =
      previous?.result === result && previous.layout.width === width
        ? previous.layout
        : prepareLoraWallLayout(
            result.options.map((option) =>
              option.kind === 'lora' && option.item.thumbnail.status === 'ready'
                ? option.item.thumbnail.width / option.item.thumbnail.height
                : 0.72,
            ),
            width,
          );
    const range = loraWallVisibleRange(layout, top, height);
    if (previous?.layout === layout && previous.from === range.from && previous.to === range.to)
      return;
    const references = result.options
      .slice(range.from, range.to)
      .flatMap((option) =>
        option.kind === 'lora' && option.item.thumbnail.status === 'ready'
          ? [option.item.thumbnail]
          : [],
      );
    if (
      this.disposed ||
      this.measurementRevision !== revision ||
      this.view.state.field(completionSession)?.result !== result ||
      this.view.state.field(completionSession)?.presentation !== presentation
    )
      return;
    this.view.dispatch({
      effects: installCompletionWall.of({
        result,
        layout,
        ...range,
        references,
      }),
      annotations: Transaction.addToHistory.of(false),
    });
  }
  /** Keep only viewport/overscan tiles plus the selected accessible option; scrolling never truncates the ranked candidate set. */
  private project(): void {
    if (this.disposed) return;
    const session = this.view.state.field(completionSession);
    if (!session) return;
    this.scrollbar.session(session.result, session.presentation);
    if (this.disposed || this.view.state.field(completionSession) !== session) return;
    const model = requireValue(session.result.options[0], 'completion-tooltip').kind === 'lora';
    // Plain rows survive a new presentation and are relabeled in place below; model walls rebuild.
    const relabel = this.presentation !== session.presentation;
    if (relabel && (model || this.dom.classList.contains('sp-completion-wall'))) {
      this.nodes.clear();
      this.content.replaceChildren();
    }
    this.presentation = session.presentation;
    this.dom.classList.toggle('sp-completion-wall', model);
    this.result = session.result;
    this.viewport.id = session.id;
    const wall = this.view.state.field(completionWall);
    const indices = new Set<number>();
    if (model) {
      this.content.style.height = `${wall?.layout.height ?? 0}px`;
      if (wall?.result === session.result)
        for (let index = wall.from; index < wall.to; index++) indices.add(index);
      indices.add(session.selected);
    } else for (let index = 0; index < session.result.options.length; index++) indices.add(index);
    for (const [index, node] of this.nodes)
      if (!indices.has(index)) {
        node.dom.remove();
        this.nodes.delete(index);
      }
    for (const index of indices) {
      const option = requireValue(session.result.options[index], 'completion-tooltip');
      const count = session.result.options.length;
      let node = this.nodes.get(index);
      if (node && relabel && !relabelCompletionRow(node, option, session.id, index, count)) {
        const replaced = node.dom;
        node = createCompletionOption(option, session.id, index, count);
        this.nodes.set(index, node);
        replaced.replaceWith(node.dom);
      }
      if (!node) {
        node = createCompletionOption(option, session.id, index, count);
        this.nodes.set(index, node);
        this.content.append(node.dom);
      }
      node.option = option;
      node.dom.setAttribute('aria-selected', String(index === session.selected));
      const box = model && wall?.result === session.result ? wall.layout.boxes[index] : null;
      if (model) {
        node.dom.style.visibility = box ? 'visible' : 'hidden';
        if (box)
          Object.assign(node.dom.style, {
            left: `${box.x}px`,
            top: `${box.y}px`,
            width: `${box.width}px`,
            height: `${box.height}px`,
          });
      }
    }
    this.paintMedia();
    this.titles.refresh();
  }
  /** Paint only changed actual prepared raster identities, never decode or traverse catalog metadata. */
  private paintMedia(): void {
    const media = this.view.state.field(loraMedia);
    for (const node of this.nodes.values()) paintCompletionMedia(node, media);
  }
}
/** Stable tooltip factory permits CodeMirror to reuse the finite DOM owner across native selection changes. */
function createTooltip(view: EditorView): TooltipView {
  return new CompletionTooltip(view);
}
/** The logical-document UTF-16 caret anchors supported tooltip placement; query bounds retain source-edit authority. */
export const completionTooltips: Extension = [
  completionWall,
  showTooltip.compute([completionSession, 'selection'], (state) => {
    const session = state.field(completionSession);
    return session ? { pos: state.selection.main.head, create: createTooltip } : null;
  }),
  EditorView.contentAttributes.compute([completionSession], (state): Record<string, string> => {
    const session: CompletionSession | null = state.field(completionSession);
    return session
      ? {
          'aria-autocomplete': 'list',
          'aria-controls': session.id,
          'aria-activedescendant': `${session.id}-${session.selected}`,
          'aria-busy': String(session.phase === 'refreshing'),
        }
      : { 'aria-autocomplete': 'list' };
  }),
  completionTheme,
];
