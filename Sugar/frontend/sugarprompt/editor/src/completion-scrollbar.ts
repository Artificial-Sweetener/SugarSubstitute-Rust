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
/** A finite thumb adapts the owned native completion viewport without another scroll or selection owner. */
import type { PreparedCompletionResult } from './prompt-completion-model.js';

/** A displayed thumb uses the native Fluent track inset while retaining the browser's scrollTop as its only value. */
export function completionThumb(
  height: number,
  extent: number,
  top: number,
): {
  readonly length: number;
  readonly top: number;
  readonly travel: number;
} | null {
  if (![height, extent, top].every(Number.isFinite) || height <= 0 || extent <= height) return null;
  const track = Math.max(0, height - 30);
  const length = Math.min(track, Math.max(30, Math.floor((track * height) / extent)));
  const travel = track - length;
  if (travel <= 0) return null;
  return {
    length,
    travel,
    top:
      15 + Math.floor((Math.max(0, Math.min(extent - height, top)) / (extent - height)) * travel),
  };
}

/** Native READ geometry is bound to the exact completion publication and resource generation. */
export interface CompletionScrollGeometry {
  readonly result: PreparedCompletionResult;
  readonly presentation: symbol;
  readonly generation: number;
  readonly height: number;
  readonly extent: number;
  readonly top: number;
  readonly scale: number;
  readonly left: number;
  readonly physicalTop: number;
  readonly width: number;
}

/** One captured pointer retains only its immutable native baseline and latest input coordinate. */
interface ScrollDrag {
  readonly pointer: number;
  readonly result: PreparedCompletionResult;
  readonly presentation: symbol;
  readonly startY: number;
  y: number;
  baseline: CompletionScrollGeometry | null;
}

/** Native scrolling, keyboard selection and accessibility stay with the same listbox viewport. */
export class CompletionScrollbar {
  readonly dom: HTMLElement;
  private readonly thumb: HTMLElement;
  private readonly viewport: HTMLElement;
  private readonly measure: () => void;
  private result: PreparedCompletionResult | null = null;
  private presentation: symbol | null = null;
  private generation = 0;
  private drag: ScrollDrag | null = null;
  private geometry: CompletionScrollGeometry | null = null;
  private disposed = false;

  /** Acquire only an inert zero-height carrier and one owned pointer target inside the native scroll ancestry. */
  constructor(viewport: HTMLElement, measure: () => void) {
    this.viewport = viewport;
    this.measure = measure;
    this.dom = viewport.ownerDocument.createElement('div');
    this.thumb = viewport.ownerDocument.createElement('div');
    try {
      this.dom.className = 'sp-completion-scrollbar';
      this.dom.setAttribute('aria-hidden', 'true');
      this.thumb.className = 'sp-completion-scroll-thumb';
      this.thumb.hidden = true;
      this.dom.append(this.thumb);
      this.thumb.addEventListener('pointerdown', this.started);
      this.thumb.addEventListener('pointermove', this.moved);
      this.thumb.addEventListener('pointerup', this.ended);
      this.thumb.addEventListener('pointercancel', this.ended);
      this.thumb.addEventListener('lostpointercapture', this.ended);
    } catch (error) {
      this.fail(error);
    }
  }

  /** Candidate replacement retires the captured pointer before a new publication can consume its geometry. */
  session(result: PreparedCompletionResult, presentation: symbol): void {
    if (this.disposed) return;
    if (this.result === result && this.presentation === presentation) return;
    this.result = result;
    this.presentation = presentation;
    this.invalidate();
  }

  /** Actual resize/placement invalidates geometry; ordinary native scroll does not retire its own drag. */
  invalidate(): void {
    if (this.disposed) return;
    this.generation++;
    this.geometry = null;
    this.cancel();
  }

  /** Capture actual dimensions only inside the existing Tooltip CodeMirror READ phase. */
  read(): CompletionScrollGeometry | null {
    const result = this.result,
      presentation = this.presentation,
      generation = this.generation;
    if (this.disposed || !result || !presentation) return null;
    const height = this.viewport.clientHeight,
      extent = this.viewport.scrollHeight,
      top = this.viewport.scrollTop,
      rect = this.viewport.getBoundingClientRect(),
      physical = rect.height;
    if (
      this.disposed ||
      this.result !== result ||
      this.presentation !== presentation ||
      this.generation !== generation
    )
      return null;
    if (
      ![height, extent, top, physical].every(Number.isFinite) ||
      height <= 0 ||
      extent < height ||
      physical <= 0
    )
      return null;
    if (![rect.left, rect.top, rect.width].every(Number.isFinite) || rect.width <= 0) return null;
    return {
      result,
      presentation,
      generation,
      height,
      extent,
      top,
      scale: physical / height,
      left: rect.left,
      physicalTop: rect.top,
      width: rect.width,
    };
  }

  /** Project one current native measurement; a released pre-baseline pointer cannot be revived here. */
  project(geometry: CompletionScrollGeometry | null): void {
    if (!geometry || !this.current(geometry)) return;
    const previous = this.geometry;
    if (
      previous &&
      (previous.left !== geometry.left ||
        previous.physicalTop !== geometry.physicalTop ||
        previous.width !== geometry.width ||
        previous.height !== geometry.height ||
        previous.scale !== geometry.scale ||
        previous.extent !== geometry.extent)
    )
      this.cancel();
    if (!this.current(geometry)) return;
    this.geometry = geometry;
    const thumb = completionThumb(geometry.height, geometry.extent, geometry.top);
    this.thumb.hidden = !thumb;
    if (!this.current(geometry)) return;
    if (!thumb) {
      this.cancel();
      return;
    }
    this.thumb.style.height = `${thumb.length}px`;
    if (!this.current(geometry)) return;
    this.thumb.style.top = `${thumb.top}px`;
    if (!this.current(geometry)) return;
    const drag = this.drag;
    if (
      drag &&
      !drag.baseline &&
      drag.result === geometry.result &&
      drag.presentation === geometry.presentation
    ) {
      drag.baseline = geometry;
      if (drag.y !== drag.startY) this.scroll(drag);
    }
  }

  /** Release every owned listener, pointer and carrier independently after retiring publication authority. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.generation++;
    this.result = null;
    this.presentation = null;
    this.geometry = null;
    const causes: unknown[] = [];
    for (const release of [
      () => this.cancel(),
      () => this.thumb.removeEventListener('pointerdown', this.started),
      () => this.thumb.removeEventListener('pointermove', this.moved),
      () => this.thumb.removeEventListener('pointerup', this.ended),
      () => this.thumb.removeEventListener('pointercancel', this.ended),
      () => this.thumb.removeEventListener('lostpointercapture', this.ended),
      () => this.dom.remove(),
    ]) {
      try {
        release();
      } catch (error) {
        causes.push(error);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length > 1)
      throw new AggregateError(causes, 'Completion scrollbar retirement failed');
  }

  /** Pure publication checks precede and follow callback-capable native attachment observations. */
  private current(geometry: CompletionScrollGeometry): boolean {
    const matches = () =>
      !this.disposed &&
      this.result === geometry.result &&
      this.presentation === geometry.presentation &&
      this.generation === geometry.generation;
    return (
      matches() &&
      this.viewport.isConnected &&
      this.dom.parentElement === this.viewport &&
      matches()
    );
  }

  /** A primary press reserves one exact pointer; fresh native geometry is obtained by the existing measure queue. */
  private readonly started = (event: PointerEvent): void => {
    if (
      this.disposed ||
      event.button !== 0 ||
      this.drag ||
      !this.result ||
      !this.presentation ||
      this.thumb.hidden
    )
      return;
    const drag: ScrollDrag = {
      pointer: event.pointerId,
      result: this.result,
      presentation: this.presentation,
      startY: event.clientY,
      y: event.clientY,
      baseline: null,
    };
    this.drag = drag;
    try {
      event.preventDefault();
      if (this.disposed || this.drag !== drag) return;
      this.thumb.setPointerCapture(event.pointerId);
      if (this.drag !== drag || this.disposed) {
        if (!this.ownsPointer(drag.pointer) && this.thumb.hasPointerCapture(drag.pointer))
          this.thumb.releasePointerCapture(drag.pointer);
        return;
      }
      this.measure();
    } catch (error) {
      this.fail(error);
    }
  };

  /** Coalesce input before baseline admission; current admitted deltas only assign the native scrollTop. */
  private readonly moved = (event: PointerEvent): void => {
    const drag = this.drag;
    if (!drag || event.pointerId !== drag.pointer) return;
    drag.y = event.clientY;
    try {
      this.scroll(drag);
    } catch (error) {
      this.fail(error);
    }
  };

  /** Pointer completion is latched before release callbacks and before any pending native READ can settle. */
  private readonly ended = (event: PointerEvent): void => {
    if (this.drag?.pointer !== event.pointerId) return;
    try {
      this.cancel();
    } catch (error) {
      this.fail(error);
    }
  };

  /** A retained scale/extent baseline never derives a competing scroll value or observes native layout in input. */
  private scroll(drag: ScrollDrag): void {
    const baseline = drag.baseline;
    if (!baseline || this.drag !== drag || !this.current(baseline) || this.drag !== drag) return;
    const maximum = baseline.extent - baseline.height,
      thumb = completionThumb(baseline.height, baseline.extent, baseline.top);
    if (!thumb) return;
    this.viewport.scrollTop = Math.max(
      0,
      Math.min(
        maximum,
        baseline.top + (((drag.y - drag.startY) / baseline.scale) * maximum) / thumb.travel,
      ),
    );
    if (this.drag === drag && this.current(baseline) && this.drag === drag) this.measure();
  }

  /** Observe the current pointer after native capture callbacks without reviving a retired record. */
  private ownsPointer(pointer: number): boolean {
    return this.drag?.pointer === pointer;
  }

  /** Clear the captured record before releasing native capture, preserving reentrant newer pointer ownership. */
  private cancel(): void {
    const drag = this.drag;
    this.drag = null;
    if (!drag) return;
    let captured = true;
    const causes: unknown[] = [];
    try {
      captured = this.thumb.hasPointerCapture(drag.pointer);
    } catch (error) {
      causes.push(error);
    }
    if (captured && !this.ownsPointer(drag.pointer)) {
      try {
        this.thumb.releasePointerCapture(drag.pointer);
      } catch (error) {
        causes.push(error);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length > 1) throw new AggregateError(causes, 'Completion pointer retirement failed');
  }

  /** Native event/acquisition failures retire this finite adapter and preserve the raw primary before cleanup causes. */
  private fail(primary: unknown): never {
    try {
      this.destroy();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'Completion scrollbar failed');
    }
    throw primary;
  }
}
