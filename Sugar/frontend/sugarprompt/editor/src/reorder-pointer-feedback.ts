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
/** Complete finite pointer feedback owns external, noneditable proxy and insertion-marker DOM only. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMarkerRect } from './reorder-geometry.js';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
/** Measured typography and bounded public theme attributes travel with the external paint surface. */
interface ProxyAppearance {
  readonly font: string;
  readonly lineHeight: string;
  readonly color: string;
  readonly surface: string;
  readonly error: string;
  readonly diagnostics: string | null;
  readonly documentDiagnostics: string | null;
}
/** Measured feedback never authors source, target intent, selection or history. */
export class ReorderPointerFeedback {
  private readonly view: EditorView;
  private readonly window: Window;
  private frame: number | null = null;
  private generation = 0;
  private paintedCoordinates: {
    readonly x: number;
    readonly y: number;
  } | null = null;
  private pending: {
    readonly coordinates: {
      readonly x: number;
      readonly y: number;
    };
    readonly current: () => boolean;
    readonly observe: (action: () => void) => void;
  } | null = null;
  private proxy: HTMLElement | null = null;
  private surface: HTMLElement | null = null;
  private position: CSSStyleDeclaration | null = null;
  private appearance: ProxyAppearance | null = null;
  private marker: HTMLElement | null = null;
  private markerPending = false;
  private grab: ReorderGrabGeometry | null = null;
  /** Retain only this mounted view's external paint lifetime. */
  constructor(view: EditorView) {
    this.view = view;
    const window = view.dom.ownerDocument.defaultView;
    if (!window) throw new Error('reorder-feedback-window-unavailable');
    this.window = window;
  }
  /** Read actual editor appearance only in its entry measurement phase, before any proxy mounts. */
  measureAppearance(): void {
    const style = requireValue(
      this.view.dom.ownerDocument.defaultView,
      'reorder-pointer-feedback',
    ).getComputedStyle(this.view.contentDOM);
    this.appearance = {
      font: style.font,
      lineHeight: style.lineHeight,
      color: style.color,
      surface: style.getPropertyValue('--sugarprompt-lora-surface'),
      error: style.getPropertyValue('--sugarprompt-error-foreground'),
      diagnostics: this.view.dom.getAttribute('data-diagnostic-source'),
      documentDiagnostics: this.view.dom.getAttribute('data-document-diagnostic-source'),
    };
  }
  /** Adopt only already measured inert native paint; canonical native rich layout never reflows into an isolated fragment. */
  begin(
    paint: ReorderGrabPaint,
    coordinates: {
      readonly x: number;
      readonly y: number;
    },
  ): void {
    try {
      this.close();
    } catch (cause) {
      try {
        paint.node.remove();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'reorder-feedback-grab-retirement-failed');
      }
      throw cause;
    }
    const appearance = this.appearance;
    const proxy = paint.node,
      grab = paint.geometry;
    this.proxy = proxy;
    this.grab = grab;
    if (!appearance) throw new Error('reorder-proxy-appearance-unmeasured');
    const surface = this.view.dom.ownerDocument.createElement('span');
    this.surface = surface;
    const position = surface.style;
    this.position = position;
    surface.className = `${this.view.themeClasses} sp-reorder-drag-surface`.trim();
    // The inherited CodeMirror theme root requires relative positioning with important priority.
    surface.style.setProperty('position', 'fixed', 'important');
    Object.assign(surface.style, {
      display: 'block',
      left: '0px',
      top: '0px',
      boxSizing: 'border-box',
      border: '0',
      padding: '0',
      margin: '0',
      background: 'transparent',
      boxShadow: 'none',
      pointerEvents: 'none',
      zIndex: '100',
      width: `${grab.width}px`,
      height: `${grab.height}px`,
      overflow: 'visible',
      font: appearance.font,
      lineHeight: appearance.lineHeight,
      color: appearance.color,
    });
    surface.style.setProperty('--sugarprompt-lora-surface', appearance.surface);
    surface.style.setProperty('--sugarprompt-error-foreground', appearance.error);
    for (const [name, value] of [
      ['data-diagnostic-source', appearance.diagnostics],
      ['data-document-diagnostic-source', appearance.documentDiagnostics],
    ] as const)
      if (value !== null) surface.setAttribute(name, value);
    surface.setAttribute('aria-hidden', 'true');
    surface.inert = true;
    Object.assign(proxy.style, {
      position: 'absolute',
      left: '0px',
      top: '0px',
    });
    surface.append(proxy);
    this.move(coordinates);
    if (this.surface !== surface || this.proxy !== proxy || this.position !== position) return;
    this.view.dom.ownerDocument.body.append(surface);
  }
  /** Donor placement probes the translated held rectangle center using the same admitted press hotspot as proxy positioning. */
  probe(coordinates: { readonly x: number; readonly y: number }): {
    readonly x: number;
    readonly y: number;
  } | null {
    const grab = this.grab;
    return grab
      ? {
          x: coordinates.x - grab.grabX + grab.width / 2,
          y: coordinates.y - grab.grabY + grab.height / 2,
        }
      : null;
  }
  /** One latest-coordinate frame moves only prepared external paint; collision and final-drop authority remain with measurement. */
  follow(
    coordinates: {
      readonly x: number;
      readonly y: number;
    },
    current: () => boolean,
    observe: (action: () => void) => void,
  ): void {
    const surface = this.surface;
    if (!surface || !this.grab) return;
    if (
      this.frame === null &&
      this.paintedCoordinates?.x === coordinates.x &&
      this.paintedCoordinates.y === coordinates.y
    )
      return;
    this.pending = { coordinates, current, observe };
    if (this.frame !== null) return;
    const generation = this.generation;
    try {
      this.frame = this.window.requestAnimationFrame(() => {
        if (generation !== this.generation || this.surface !== surface) return;
        this.frame = null;
        const pending = this.pending;
        this.pending = null;
        if (!pending) return;
        try {
          pending.observe(() => {
            if (
              pending.current() &&
              generation === this.generation &&
              this.surface === surface &&
              this.pending === null
            )
              this.move(pending.coordinates);
          });
        } catch (cause) {
          if (generation !== this.generation || this.surface !== surface) throw cause;
          this.fail(cause);
        }
      });
    } catch (cause) {
      if (generation !== this.generation || this.surface !== surface) throw cause;
      this.fail(cause);
    }
  }
  /** A failed frame cannot retain paint or replace its raw primary with cancellation/removal errors. */
  private fail(cause: unknown): never {
    try {
      this.close();
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'reorder-feedback-frame-retirement-failed');
    }
    throw cause;
  }
  /** Translate the owned fixed block from its zero origin without changing the captured inline paint flow. */
  private move(coordinates: { readonly x: number; readonly y: number }): void {
    const position = this.position,
      grab = this.grab,
      surface = this.surface,
      generation = this.generation;
    if (!position || !grab) return;
    position.transform = `translate(${coordinates.x - grab.grabX}px, ${coordinates.y - grab.grabY}px)`;
    if (
      this.position === position &&
      this.grab === grab &&
      this.surface === surface &&
      this.generation === generation
    )
      this.paintedCoordinates = coordinates;
  }
  /** Display only a current measured/clipped boundary; absent geometry retires the former marker. */
  mark(rect: ReorderMarkerRect | null, pending = false): void {
    if (!this.proxy || !rect) {
      this.clearMarker();
      return;
    }
    if (!this.marker) {
      const marker = this.view.dom.ownerDocument.createElement('span');
      marker.className = 'sp-reorder-insertion-marker';
      marker.setAttribute('aria-hidden', 'true');
      Object.assign(marker.style, {
        position: 'fixed',
        pointerEvents: 'none',
        zIndex: '101',
        background: 'Highlight',
      });
      this.marker = marker;
      this.view.dom.ownerDocument.body.append(marker);
    }
    this.markerPending = pending;
    Object.assign(this.marker.style, {
      opacity: pending ? '0.52' : '1',
      left: `${rect.left}px`,
      top: `${rect.top}px`,
      width: `${rect.right - rect.left}px`,
      height: `${rect.bottom - rect.top}px`,
    });
  }
  /** A newer unmeasured pointer intent dims the last accepted marker once; geometry retirement remains explicit. */
  pendingMarker(): void {
    if (this.marker && !this.markerPending) {
      this.markerPending = true;
      this.marker.style.opacity = '0.52';
    }
  }
  /** Superseded geometry cannot remain visible while the next read is pending. */
  clearMarker(): void {
    const marker = this.marker;
    this.marker = null;
    this.markerPending = false;
    marker?.remove();
  }
  /** Retire both captured nodes even when an earlier native removal fails. */
  close(): void {
    const proxy = this.proxy,
      marker = this.marker,
      surface = this.surface,
      frame = this.frame;
    ++this.generation;
    this.frame = null;
    this.pending = null;
    this.paintedCoordinates = null;
    this.proxy = null;
    this.marker = null;
    this.markerPending = false;
    this.surface = null;
    this.position = null;
    this.grab = null;
    const causes: unknown[] = [];
    for (const release of [
      () => {
        if (frame !== null) this.window.cancelAnimationFrame(frame);
      },
      () => marker?.remove(),
      () => surface?.remove(),
      () => proxy?.remove(),
    ]) {
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Reorder pointer feedback cleanup failed');
  }
}
