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
/** Native geometry publication uses CodeMirror measurement and owns no alternate layout engine. */
import { requireValue } from '../../core/src/required.js';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { EditorLayoutMetrics } from './contracts.js';
import { changesExactSource } from './exact-source.js';
/** One coalesced read/write task belongs to the mounted native view. */
class NativeLayout {
  private readonly revision: () => number;
  private readonly view: EditorView;
  private readonly observe: ((metrics: EditorLayoutMetrics) => void) | undefined;
  private previous: EditorLayoutMetrics | undefined;
  private resize: ResizeObserver | null = null;
  private queued = false;
  private disposed = false;
  /** Queue initial measurement after the native view mounts. */
  constructor(
    view: EditorView,
    observe: ((metrics: EditorLayoutMetrics) => void) | undefined,
    revision: () => number,
  ) {
    this.view = view;
    this.revision = revision;
    this.observe = observe;
    try {
      if (observe && typeof ResizeObserver === 'function') {
        const resize = new ResizeObserver(() => this.invalidate());
        this.resize = resize;
        // Host width and resulting native content height can change without a CodeMirror ViewUpdate.
        resize.observe(view.dom);
        resize.observe(view.contentDOM);
      }
      this.queue();
    } catch (primary) {
      try {
        this.destroy();
      } catch (cleanup) {
        throw new AggregateError(
          [primary, cleanup],
          'Native layout invalidation/retirement failed',
        );
      }
      throw primary;
    }
  }
  /** Native layout invalidation covers text, wrapping, projection, resize and viewport changes. */
  update(update: ViewUpdate): void {
    if (
      update.transactions.some(changesExactSource) ||
      update.geometryChanged ||
      update.viewportChanged
    )
      this.queue();
  }
  /** A replaced baseline or destroyed editor cannot publish its retired geometry. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.previous = undefined;
    const resize = this.resize;
    this.resize = null;
    resize?.disconnect();
  }
  /** Native callbacks own scheduling failures and retire their observer before exposing raw causes. */
  private invalidate(): void {
    try {
      this.queue();
    } catch (primary) {
      try {
        this.destroy();
      } catch (cleanup) {
        throw new AggregateError(
          [primary, cleanup],
          'Native layout invalidation/retirement failed',
        );
      }
      throw primary;
    }
  }
  /** Measure only prepared native layout and emit actual changed output once. */
  private queue(): void {
    if (!this.observe || this.queued || this.disposed) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      /** CSS-local pixels retain their meaning under transformed host containers. */
      read: () => {
        const parent = this.view.dom.parentElement;
        if (this.disposed || !parent) return null;
        const style = requireValue(
          this.view.dom.ownerDocument.defaultView,
          'native-layout',
        ).getComputedStyle(parent);
        return Object.freeze({
          revision: this.revision(),
          contentHeight: this.view.contentHeight / this.view.scaleY,
          lineHeight: this.view.defaultLineHeight / this.view.scaleY,
          verticalPadding:
            (this.view.documentPadding.top + this.view.documentPadding.bottom) / this.view.scaleY,
          outerPadding:
            Number.parseFloat(style.borderTopWidth) +
            Number.parseFloat(style.borderBottomWidth) +
            Number.parseFloat(style.paddingTop) +
            Number.parseFloat(style.paddingBottom),
          viewportHeight: this.view.scrollDOM.clientHeight,
          viewportWidth: parent.clientWidth,
          scaleY: this.view.scaleY,
        });
      },
      /** Vue may compose shell sizing after measurement; it never replaces native geometry. */
      write: (metrics) => {
        this.queued = false;
        if (
          !metrics ||
          this.disposed ||
          (this.previous &&
            Object.keys(metrics).every(
              (key) =>
                metrics[key as keyof EditorLayoutMetrics] ===
                requireValue(this.previous, 'native-layout')[key as keyof EditorLayoutMetrics],
            ))
        )
          return;
        this.previous = metrics;
        this.observe?.(metrics);
      },
    });
  }
}
/** The supported mount facade supplies one optional geometry observer for this view lifetime. */
export function nativeLayout(
  observe: ((metrics: EditorLayoutMetrics) => void) | undefined,
  revision: () => number,
) {
  return ViewPlugin.define((view) => new NativeLayout(view, observe, revision));
}
