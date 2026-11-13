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
/** One finite regional rename interaction delegates source, caret and history to native commands. */
import { captureFocusHandoff, currentFocusHandoff } from './focus-handoff.js';
import { Prec, Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { validRegionName } from '../../core/src/index.js';
import { editorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { richRendering } from './presentation.js';
import { regionAtPosition, type RegionTarget } from './region-index.js';
import {
  currentRegionName,
  regionNameTransactions,
  type PreparedRegionName,
} from './region-command.js';
import { regionDraft, setRegionDraft } from './region-draft.js';
import { measuredTypography } from './native-typography.js';
import { RegionNameInput, type RegionNameGeometry } from './region-name-input.js';

/** The mounted view owns only current intent, a visible draft and finite measurement/listener callbacks. */
class RegionNames {
  private readonly view: EditorView;
  private target: PreparedRegionName | null = null;
  private input: RegionNameInput | null = null;
  private generation = 0;
  private session = 0;
  private disposed = false;
  private frame: number | null = null;
  /** Bind the interaction to one existing native editor lifetime. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Replacement, permissions, raw mode and native composition retire the current draft. */
  update(update: ViewUpdate): void {
    if (!this.target) return;
    if (!currentRegionName(update.state, this.target) || !this.allowed()) this.close();
    else if (update.geometryChanged || update.viewportChanged) this.measure();
  }
  /** Keyboard intent resolves prepared inclusive token bounds rather than parsing at the caret. */
  key(event: KeyboardEvent): boolean {
    if (
      event.key !== 'F2' ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      !this.allowed()
    )
      return false;
    const targets = this.view.state.field(semanticStructures).native?.regions;
    const target = targets && regionAtPosition(targets, this.view.state.selection.main.head);
    if (!target) return false;
    event.preventDefault();
    this.begin(target);
    return true;
  }
  /** The entire structural row admits a real double press; ordinary pointer placement remains native. */
  press(event: MouseEvent): boolean {
    if (
      event.button !== 0 ||
      event.detail < 2 ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      !this.allowed()
    )
      return false;
    const row =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>('.sp-region-separator')
        : null;
    if (!row) return false;
    const ordinal = Number(row.dataset.regionIndex),
      target = this.view.state
        .field(semanticStructures)
        .native?.regions.find(
          (member) => member.scopeId === row.dataset.regionScope && member.ordinal === ordinal,
        );
    if (
      !target ||
      this.view.state.field(semanticStructures).native?.sourceIdentity !==
        this.view.state.field(editorFreshness).source
    )
      return false;
    event.preventDefault();
    this.begin(target);
    return true;
  }
  /** Incomplete prompt composition and disabled mutation exclude name authoring. */
  private allowed(): boolean {
    return (
      !this.disposed &&
      !this.view.state.readOnly &&
      this.view.state.facet(richRendering) &&
      !this.view.compositionStarted &&
      !this.view.composing
    );
  }
  /** A current prepared target is captured before its measured native draft appears. */
  private begin(target: RegionTarget): void {
    const session = this.session + 1;
    this.close();
    if (this.disposed || session !== this.session || !this.allowed()) return;
    this.target = { target, freshness: this.view.state.field(editorFreshness) };
    const prepared = this.target;
    if (!currentRegionName(this.view.state, prepared)) {
      this.target = null;
      return;
    }
    this.view.dispatch({
      effects: setRegionDraft.of({
        freshness: this.target.freshness,
        scopeId: target.scopeId,
        ordinal: target.ordinal,
        width: 0,
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    if (
      this.disposed ||
      session !== this.session ||
      this.target !== prepared ||
      !currentRegionName(this.view.state, prepared)
    )
      return;
    window.addEventListener('scroll', this.layout, true);
    window.addEventListener('resize', this.layout);
    this.measure();
    this.frame = requestAnimationFrame(this.trackGeometry);
  }
  /** A visible draft follows CSS transforms through bounded native measure phases until its finite lifetime closes. */
  private readonly trackGeometry = (): void => {
    if (this.disposed || !this.target) return;
    this.measure();
    this.frame = requestAnimationFrame(this.trackGeometry);
  };
  /** Native layout notifications coalesce through CodeMirror measurement. */
  private readonly layout = (): void => this.measure();
  /** Geometry reads follow the current source row, actual glyph metrics, native transforms and viewport clipping. */
  private geometry(): RegionNameGeometry | null {
    const target = this.target;
    if (!target || !currentRegionName(this.view.state, target)) return null;
    const row = this.view.contentDOM.querySelector<HTMLElement>(
      `.sp-region-separator[data-region-scope="${CSS.escape(target.target.scopeId)}"][data-region-index="${target.target.ordinal}"]`,
    );
    const label = row?.querySelector<HTMLElement>('.sp-region-label');
    if (!row || !label) return null;
    const bounds = row.getBoundingClientRect(),
      viewport = this.view.scrollDOM.getBoundingClientRect(),
      style = getComputedStyle(label);
    if (bounds.bottom < viewport.top || bounds.top > viewport.bottom) return null;
    const contentWidth = bounds.width / this.view.scaleX,
      maximumWidth = Math.max(
        1,
        Math.min(contentWidth, contentWidth - 2 * (Math.min(36, contentWidth * 0.2) + 8) + 12),
      ),
      width = Math.max(
        1,
        Math.min(
          maximumWidth,
          (this.input?.textWidth() ?? label.getBoundingClientRect().width / this.view.scaleX) + 12,
        ),
      );
    return {
      center: bounds.left + bounds.width / 2,
      top: bounds.top,
      height: bounds.height,
      maximumWidth,
      width,
      scaleX: this.view.scaleX,
      scaleY: this.view.scaleY,
      typography: measuredTypography(label),
      color: style.color,
    };
  }
  /** Publication follows native reads and rejects queued callbacks from retired or replaced sessions. */
  private measure(): void {
    if (!this.target || this.disposed) return;
    const generation = ++this.generation;
    this.view.requestMeasure({
      key: this,
      read: () => this.geometry(),
      write: (geometry) =>
        queueMicrotask(() => {
          const target = this.target;
          if (this.disposed || !target || generation !== this.generation) return;
          if (!geometry || !this.allowed()) {
            this.finish(true, true);
            return;
          }
          const mounted = this.input === null;
          if (!this.input)
            this.input = new RegionNameInput(
              target.target.separator.name ?? '',
              (cancel, outside) => this.finish(cancel, outside),
              this.layout,
            );
          this.input.place(geometry);
          if (mounted) {
            this.input.focus();
            if (
              this.disposed ||
              generation !== this.generation ||
              this.target !== target ||
              !this.input
            )
              return;
            this.measure();
          }
          const titleWidth = Math.max(0, geometry.width - 12);
          if (this.view.state.field(regionDraft)?.width !== titleWidth)
            this.view.dispatch({
              effects: setRegionDraft.of({
                freshness: target.freshness,
                scopeId: target.target.scopeId,
                ordinal: target.target.ordinal,
                width: titleWidth,
              }),
              annotations: Transaction.addToHistory.of(false),
            });
        }),
    });
  }
  /** Close native draft intent before blur reentrancy; valid completion is one exact undoable source edit. */
  private finish(cancel: boolean, outside: boolean): void {
    const target = this.target,
      input = this.input;
    if (!target) return;
    const name = input?.element.value ?? '';
    if (!cancel && (!input || !validRegionName(name))) return;
    const state = this.view.state;
    const draft = state.field(regionDraft);
    const handoff = captureFocusHandoff(this.view);
    const session = this.session + 1;
    const current = (): boolean =>
      !this.disposed && this.session === session && this.target === null && this.input === null;
    this.close();
    /** Only the retired projection may be removed; newer source/draft/lifetime authority remains untouched. */
    const cleanDraft = (): void => {
      if (current() && draft && this.view.state.field(regionDraft) === draft)
        this.view.dispatch({
          effects: setRegionDraft.of(null),
          annotations: Transaction.addToHistory.of(false),
        });
    };
    if (
      !current() ||
      this.view.state !== state ||
      this.view.state.field(regionDraft) !== draft ||
      !currentFocusHandoff(this.view, handoff)
    ) {
      cleanDraft();
      return;
    }
    const transactions =
      !cancel && this.allowed() ? regionNameTransactions(this.view.state, target, name) : [];
    if (transactions.length) this.view.dispatch(transactions);
    if (
      !current() ||
      this.view.state !== (transactions.at(-1)?.state ?? state) ||
      !currentFocusHandoff(this.view, handoff)
    ) {
      cleanDraft();
      return;
    }
    cleanDraft();
    if (!outside && current() && this.view.state.field(regionDraft) === null)
      void handoff.restore();
  }
  /** Release all draft and geometry ownership before removing its visible native subtree. */
  private close(): void {
    this.generation++;
    this.session++;
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.target = null;
    window.removeEventListener('scroll', this.layout, true);
    window.removeEventListener('resize', this.layout);
    const input = this.input;
    this.input = null;
    input?.destroy();
  }
  /** An obsolete measurement cannot recreate a draft after native teardown. */
  destroy(): void {
    this.disposed = true;
    this.close();
  }
}

/** Supported key/pointer routing gives regional name intent priority over prompt segment selection. */
export const regionNameInteractions = Prec.high(
  ViewPlugin.fromClass(RegionNames, {
    eventHandlers: {
      /** F2 consumes only a current editable structural token. */
      keydown(event) {
        return this.key(event);
      },
      /** Capture the second press before native double-click selection expands a segment. */
      mousedown(event) {
        return this.press(event);
      },
      /** Regional row double-click belongs to its already activated native name draft. */
      dblclick(event) {
        return event.target instanceof Element && !!event.target.closest('.sp-region-separator');
      },
    },
  }),
);
