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
/** Native source-line decoration and prepared viewport geometry belong to one finite view plugin. */
import { requireValue } from '../../core/src/required.js';
import type { CanonicalRowPaintPlane } from './canonical-row-paint.js';
import { regionRailMetrics } from './region-paint.js';
import { RegionRowMarker } from './region-row-marker.js';
import { regionHover } from './region-hover.js';
import { currentCanonicalProjection } from './rich-projection.js';
import { richProjection } from './rich-projection.js';
import { reorderPreview } from './reorder-preview.js';
import { prepareReorderStoragePaint } from './reorder-storage-paint.js';
import { sceneBandIntersectsLine } from './scene-line-bands.js';
import { readCanonicalRowEdges } from './canonical-row-edges.js';
import { Facet, type Extension } from '@codemirror/state';
import {
  Decoration,
  layer,
  RectangleMarker,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import type {
  SourceLineChromeOptions,
  SourceLineRect,
  SourceLineSnapshot,
} from './source-line-contracts.js';
import { changesExactSource } from './exact-source.js';
/** Baselines and live configuration use the same immutable native intent. */
export const sourceLineChrome = Facet.define<SourceLineChromeOptions, SourceLineChromeOptions>({
  combine: (values) => values[0] ?? Object.freeze({ enabled: false, contentLeftInset: 0 }),
});
const emptyLines: readonly SourceLineRect[] = Object.freeze([]);
/** Finite mounting callbacks are configuration, never another document or geometry owner. */
interface SourceLinePublication {
  readonly revision: () => number;
  readonly observe: ((snapshot: SourceLineSnapshot) => void) | undefined;
}
const publication = Facet.define<SourceLinePublication, SourceLinePublication>({
  combine: (values) => values[0] ?? { revision: () => 0, observe: undefined },
});
/** One finite native row-presentation owner projects source chrome and prepared Scene bands through CodeMirror measurement. */
export class SourceLines {
  decorations: DecorationSet = Decoration.none;
  private epoch = 0;
  private measuredEpoch = -1;
  private queued = false;
  private disposed = false;
  private lines = emptyLines;
  private width = 0;
  private height = 0;
  private scaleX = 1;
  private scaleY = 1;
  private currentLine = -1;
  private previous: SourceLineSnapshot | undefined;
  private readonly view: EditorView;
  private readonly revision: () => number;
  private readonly observe: ((snapshot: SourceLineSnapshot) => void) | undefined;
  /** Mount one observer and schedule native layout only after the view is attached. */
  constructor(
    view: EditorView,
    revision: () => number,
    observe: ((snapshot: SourceLineSnapshot) => void) | undefined,
  ) {
    this.view = view;
    this.revision = revision;
    this.observe = observe;
    this.decorate();
    this.queue();
  }
  /** Selection/focus reuse geometry; document, wrapping, scrolling and inset invalidate it. */
  update(update: ViewUpdate): void {
    const configurationChanged =
      update.startState.facet(sourceLineChrome) !== update.state.facet(sourceLineChrome);
    const sceneChanged =
      update.startState.field(richProjection, false)?.sceneBands !==
        update.state.field(richProjection, false)?.sceneBands ||
      update.startState.field(reorderPreview, false)?.native !==
        update.state.field(reorderPreview, false)?.native;
    const geometryChanged =
      update.transactions.some(changesExactSource) ||
      update.geometryChanged ||
      update.viewportChanged ||
      configurationChanged;
    if (geometryChanged) {
      this.epoch++;
      this.queue();
    }
    if (
      geometryChanged ||
      sceneChanged ||
      update.focusChanged ||
      (update.selectionSet && this.currentLine !== this.lineIndex())
    )
      this.decorate();
    if (
      !geometryChanged &&
      (update.selectionSet || update.focusChanged || update.transactions.some(changesExactSource))
    )
      this.publish();
  }
  /** Retired baseline views cannot publish a queued measurement or keep geometry alive. */
  destroy(): void {
    this.disposed = true;
    this.lines = emptyLines;
    this.previous = undefined;
  }
  /** Scroll within an unchanged rendered viewport still invalidates scroller-local rectangles. */
  scrolled(): void {
    if (this.disposed) return;
    this.epoch++;
    this.queue();
    this.publish();
  }
  /** Native logical line numbers include CR, LF, CRLF and trailing empty lines. */
  private lineIndex(): number {
    return this.view.state.doc.lineAt(this.view.state.selection.main.head).number - 1;
  }
  /** Observations never perform layout and never expose preedit geometry as committed source. */
  snapshot(): SourceLineSnapshot {
    const status = this.view.compositionStarted
      ? 'composing'
      : this.measuredEpoch === this.epoch
        ? 'ready'
        : 'pending';
    const value: SourceLineSnapshot = {
      status,
      revision: this.revision(),
      currentLineIndex: this.lineIndex(),
      focusActive: this.view.hasFocus,
      chrome: this.view.state.facet(sourceLineChrome),
      viewportWidth: this.width,
      viewportHeight: this.height,
      scaleX: this.scaleX,
      scaleY: this.scaleY,
      lines: status === 'ready' ? this.lines : emptyLines,
    };
    return this.previous &&
      Object.keys(value).every(
        (key) =>
          value[key as keyof SourceLineSnapshot] ===
          requireValue(this.previous, 'source-lines')[key as keyof SourceLineSnapshot],
      )
      ? this.previous
      : Object.freeze(value);
  }
  /** Publish only a changed observation, retaining the actual prepared rectangle array. */
  publish(): void {
    if (this.disposed) return;
    const value = this.snapshot();
    if (value === this.previous) return;
    this.previous = value;
    this.observe?.(value);
  }
  /** Line decorations cover native wrapped rows without parsing or scanning the entire source. */
  private decorate(): void {
    this.currentLine = this.lineIndex();
    const bands = currentCanonicalProjection(this.view.state)?.sceneBands ?? [];
    const preview = this.view.state.field(reorderPreview, false);
    const replacement =
      preview?.paint && !preview.native
        ? prepareReorderStoragePaint(preview.paint).replacement
        : null;
    const chrome = this.view.state.facet(sourceLineChrome).enabled;
    if (!chrome && !bands.length) {
      this.decorations = Decoration.none;
      return;
    }
    let ranges = this.view.visibleRanges;
    if (chrome && replacement) {
      const viewport = this.view.viewport;
      if (replacement.from <= viewport.to && viewport.from <= replacement.to) {
        // Replaced text has no visibleRanges entry; its existing native line still owns chrome and the published source-line rectangle.
        const from = this.view.state.doc.lineAt(replacement.from).from;
        ranges = [...ranges, { from, to: from }];
      }
    }
    const seen = new Set<number>();
    const values = [];
    for (const range of ranges) {
      for (let from = this.view.state.doc.lineAt(range.from).from; from <= range.to; ) {
        const line = this.view.state.doc.lineAt(from);
        if (!seen.has(line.number)) {
          seen.add(line.number);
          const classes: string[] = [];
          if (chrome) {
            classes.push('sp-source-line');
            if (line.number % 2 === 0) classes.push('sp-source-alternate');
            if (line.number - 1 === this.currentLine && this.view.hasFocus)
              classes.push('sp-source-current');
          }
          if (
            (!replacement || line.to < replacement.from || replacement.to < line.from) &&
            sceneBandIntersectsLine(bands, line.from, line.to)
          )
            classes.push('sp-scene-fill');
          if (classes.length)
            values.push(Decoration.line({ class: classes.join(' ') }).range(line.from));
        }
        if (line.to === this.view.state.doc.length) break;
        from = line.to + 1;
      }
    }
    this.decorations = Decoration.set(values, true);
  }
  /** Complementary supported native Layers borrow one row owner; request-local READs preserve current geometry without cross-Layer caching. */
  rowMarkers(plane: CanonicalRowPaintPlane): readonly RectangleMarker[] {
    if (this.disposed) return [];
    const state = this.view.state,
      projection = currentCanonicalProjection(state),
      preview = state.field(reorderPreview, false);
    if (!projection) return [];
    const current = () => !this.disposed && this.view.state === state;
    const storage = preview?.paint ? prepareReorderStoragePaint(preview.paint) : null,
      replacement = preview && !preview.native ? storage?.replacement : null,
      canonicalCurrent = () => current() && !this.view.compositionStarted;
    // Ordinary composition uses the same current prepared bands as line decoration; canonical widgets retain per-delegate composition refusal.
    const rows =
      canonicalCurrent() && preview?.paint && !preview.native
        ? readCanonicalRowEdges(this.view, canonicalCurrent, plane)
        : [];
    if (!current() || (!rows && canonicalCurrent())) return [];
    if ((plane !== 'scene' || !projection.sceneBands.length) && !rows?.length) return [];
    const viewport = this.view.scrollDOM.getBoundingClientRect();
    if (!current()) return [];
    const width = this.view.scrollDOM.clientWidth * this.view.scaleX,
      left = this.view.scrollDOM.scrollLeft * this.view.scaleX,
      top = viewport.top - this.view.scrollDOM.scrollTop * this.view.scaleY,
      documentTop = plane === 'scene' && projection.sceneBands.length ? this.view.documentTop : 0,
      blocks =
        plane === 'scene' && projection.sceneBands.length ? this.view.viewportLineBlocks : [];
    if (!current()) return [];
    const markers: RectangleMarker[] = [];
    const hover = state.field(regionHover, false);
    for (const block of blocks) {
      if (!current()) return [];
      const line = state.doc.lineAt(block.from);
      if (
        (replacement && line.to >= replacement.from && line.from <= replacement.to) ||
        !sceneBandIntersectsLine(projection.sceneBands, line.from, line.to)
      )
        continue;
      markers.push(
        new RectangleMarker(
          'sp-scene-fill-paint sp-scene-native-fill-paint',
          left,
          documentTop + block.top - top,
          width,
          block.height,
        ),
      );
    }
    const native = markers.slice();
    if (!canonicalCurrent()) return current() ? native : [];
    for (const row of rows ?? []) {
      if (!current()) return [];
      if (plane === 'scene' && row.paint.sceneFill)
        markers.push(
          new RectangleMarker(
            'sp-scene-fill-paint',
            left,
            row.top - top,
            width,
            row.bottom - row.top,
          ),
        );
      const region = row.paint.region;
      if (plane === 'region' && region && row.left !== null) {
        const hovered =
            hover?.target.scopeId === region.scopeId && hover.target.ordinal === region.ordinal,
          railWidth =
            (hovered ? regionRailMetrics.hoverWidth : regionRailMetrics.width) * this.view.scaleX;
        markers.push(
          new RegionRowMarker(
            row.left -
              viewport.left +
              left +
              regionRailMetrics.axis * this.view.scaleX -
              railWidth / 2,
            row.top - top,
            railWidth,
            row.bottom - row.top,
            hovered ? region.hover : region.normal,
          ),
        );
      }
    }
    return canonicalCurrent() ? markers : current() ? native : [];
  }
  /** Coalesce geometry work inside CodeMirror's read/write phases, including scaled host containers. */
  private queue(): void {
    if (this.queued || this.disposed) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      read: () => {
        if (this.disposed) return null;
        const viewport = this.view.scrollDOM.getBoundingClientRect();
        const height = this.view.scrollDOM.clientHeight;
        const width = this.view.scrollDOM.clientWidth;
        const rows: SourceLineRect[] = [];
        for (const block of this.view.viewportLineBlocks) {
          const top = (this.view.documentTop + block.top - viewport.top) / this.view.scaleY;
          const rowHeight = block.height / this.view.scaleY;
          if (top + rowHeight > 0 && top < height)
            rows.push(
              Object.freeze({
                lineIndex: this.view.state.doc.lineAt(block.from).number - 1,
                top,
                left: 0,
                width,
                height: rowHeight,
              }),
            );
        }
        return {
          epoch: this.epoch,
          width,
          height,
          scaleX: this.view.scaleX,
          scaleY: this.view.scaleY,
          rows: Object.freeze(rows),
        };
      },
      write: (value) => {
        this.queued = false;
        if (!value || this.disposed) return;
        if (value.epoch !== this.epoch) {
          this.queue();
          return;
        }
        this.measuredEpoch = value.epoch;
        this.width = value.width;
        this.height = value.height;
        this.scaleX = value.scaleX;
        this.scaleY = value.scaleY;
        if (!this.sameRows(value.rows)) this.lines = value.rows;
        this.publish();
      },
    });
  }
  /** Actual immutable output survives redundant measurements and parent scrolling. */
  private sameRows(rows: readonly SourceLineRect[]): boolean {
    return (
      rows.length === this.lines.length &&
      rows.every((row, index) => {
        const old = requireValue(this.lines[index], 'source-lines');
        return (
          row.lineIndex === old.lineIndex &&
          row.top === old.top &&
          row.height === old.height &&
          row.width === old.width &&
          row.left === old.left
        );
      })
    );
  }
}
/** The facade reads the same plugin that owns native row decoration and finite geometry. */
export const nativeSourceLines = ViewPlugin.define(
  (view) => {
    const options = view.state.facet(publication);
    return new SourceLines(view, options.revision, options.observe);
  },
  {
    decorations: (value) => value.decorations,
    eventObservers: {
      /** CodeMirror owns editor and ancestor scroll observation and listener disposal. */
      scroll() {
        this.scrolled();
      },
    },
  },
);
/** Supported mounting supplies publication and revision through a single plugin lifetime. */
export function sourceLinesExtension(
  revision: () => number,
  observe: ((snapshot: SourceLineSnapshot) => void) | undefined,
): Extension {
  return [
    publication.of({ revision, observe }),
    nativeSourceLines,
    ...(['scene', 'region'] as const).map((plane) =>
      layer({
        above: plane === 'region',
        class: plane === 'scene' ? 'sp-source-row-paint-layer' : 'sp-source-region-paint-layer',
        /** Canonical projection and preview transitions share CodeMirror's finite layer measurement lifetime. */
        update: (update) =>
          update.startState.field(regionHover, false) !== update.state.field(regionHover, false) ||
          update.startState.field(reorderPreview, false)?.native !==
            update.state.field(reorderPreview, false)?.native ||
          update.startState.field(reorderPreview, false)?.paint !==
            update.state.field(reorderPreview, false)?.paint ||
          update.startState.field(richProjection, false)?.sceneBands !==
            update.state.field(richProjection, false)?.sceneBands,
        /** The existing line owner borrows only this exact current live widget's resources. */
        markers: (view) => view.plugin(nativeSourceLines)?.rowMarkers(plane) ?? [],
      }),
    ),
    EditorView.editorAttributes.of((view) => ({
      style: `--sugarprompt-content-inset: ${view.state.facet(sourceLineChrome).contentLeftInset}px`,
    })),
  ];
}
