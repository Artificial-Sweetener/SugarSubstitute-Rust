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
import {
  createCompletionOption,
  relabelCompletionRow,
  type CompletionOptionDOM,
} from './completion-option-dom.js';
import { acceptCompletionOption } from './prompt-completion-acceptance.js';
import type { PreparedWordCompletions } from './prompt-completion-model.js';
import { completionTheme } from './completion-theme.js';
import { CompletionScrollbar } from './completion-scrollbar.js';
import { createModelCompletionTooltip } from './model-completion-tooltip.js';
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
  private readonly scrollbar: CompletionScrollbar;
  private result: PreparedWordCompletions | null = null;
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
    let observer: ResizeObserver | null = null;
    try {
      this.viewport.append(this.scrollbar.dom, this.content);
      this.dom.append(this.viewport);
      this.viewport.addEventListener('scroll', this.scrolled);
      this.viewport.addEventListener('mousedown', this.pressed);
      this.viewport.addEventListener('focus', this.focused);
      this.observer = observer = new ResizeObserver(() => {
        try {
          this.scrollbar.invalidate();
          this.measure();
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
      }
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
  /** Independent list scrolling changes only the derived scrollbar. */
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
            let top = measurement.top;
            const optionTop = measurement.optionTop,
              optionHeight = measurement.optionHeight;
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
  /** Relabel the plain rows in place for the session's current word candidates. */
  private project(): void {
    if (this.disposed) return;
    const session = this.view.state.field(completionSession);
    if (!session || session.result.surface !== 'words') return;
    this.scrollbar.session(session.result, session.presentation);
    if (this.disposed || this.view.state.field(completionSession) !== session) return;
    // Rows survive a new presentation and are relabeled in place below.
    const relabel = this.presentation !== session.presentation;
    this.presentation = session.presentation;
    this.result = session.result;
    this.viewport.id = session.id;
    const count = session.result.options.length;
    for (const [index, node] of this.nodes)
      if (index >= count) {
        node.dom.remove();
        this.nodes.delete(index);
      }
    for (const [index, option] of session.result.options.entries()) {
      let node = this.nodes.get(index);
      if (node && relabel) relabelCompletionRow(node, option, session.id, index, count);
      if (!node) {
        node = createCompletionOption(option, session.id, index, count);
        this.nodes.set(index, node);
        this.content.append(node.dom);
      }
      node.option = option;
      node.dom.setAttribute('aria-selected', String(index === session.selected));
    }
  }
}
/** Stable tooltip factory permits CodeMirror to reuse the finite DOM owner across native selection changes. */
function createTooltip(view: EditorView): TooltipView {
  return new CompletionTooltip(view);
}
/** The logical-document UTF-16 caret anchors supported tooltip placement; query bounds retain source-edit authority. */
export const completionTooltips: Extension = [
  // Words and models each have one surface; a session shows the one its result names.
  showTooltip.compute([completionSession, 'selection'], (state) => {
    const session = state.field(completionSession);
    return session
      ? {
          pos: state.selection.main.head,
          create:
            session.result.surface === 'models' ? createModelCompletionTooltip : createTooltip,
        }
      : null;
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
