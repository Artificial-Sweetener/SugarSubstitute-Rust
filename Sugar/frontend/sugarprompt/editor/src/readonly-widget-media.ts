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
/** Finite readonly canonical paint borrows prepared raster data without acquiring or disposing it. */
import type { LoraRaster } from './lora-media-contracts.js';

/** Direct expected cancellation during canonical construction; registration misuse remains an invariant error. */
export class ReadonlyMediaRetired extends Error {
  /** Preserve an explicit internal retirement identity without source or raster payload. */
  constructor() {
    super('readonly-media-registration-retired');
    this.name = 'ReadonlyMediaRetired';
  }
}
/** One admitted batch owns bounded prepared pixels; native READ still certifies complete viewport groups before reveal. */
export interface ReadonlyMediaPlan {
  /** Empty batches require no additional browser WRITE/READ cycle. */
  readonly empty: boolean;
  /** Expected retirement rejects publication; unexpected paint failures remain observable. */
  hydrate(): boolean;
}
/** Whole-mount preflight may defer without consuming the owner's one admitted plan. */
export type ReadonlyMediaPreflight =
  | { readonly kind: 'ready'; readonly plan: ReadonlyMediaPlan }
  | { readonly kind: 'deferred' }
  | { readonly kind: 'retired' };
/** A separately allocated canvas remains releasable after constructor or setter reentry retires its original owner. */
function releaseCanvas(canvas: HTMLCanvasElement): void {
  const failures: unknown[] = [];
  for (const release of [
    () => canvas.remove(),
    () => {
      canvas.width = 0;
    },
    () => {
      canvas.height = 0;
    },
  ]) {
    try {
      release();
    } catch (cause) {
      failures.push(cause);
    }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'readonly-canvas-retirement-failed');
}
/** A canonical absolute backing retains a borrowed prepared raster and at most one owned duplicate buffer. */
interface DeferredReadonlyMedia {
  readonly parent: HTMLElement;
  readonly raster: LoraRaster;
  canvas: HTMLCanvasElement | null;
}
/** An admitted descriptor freezes the exact borrowed raster dimensions used by both allocation paths. */
interface AdmittedReadonlyMedia {
  readonly entry: DeferredReadonlyMedia;
  readonly width: number;
  readonly height: number;
}
/** One dimension/budget owner admits prepared descriptors without consulting geometry, source or external currentness. */
function admitReadonlyMedia(
  entries: readonly DeferredReadonlyMedia[],
): readonly AdmittedReadonlyMedia[] | null {
  const admitted: AdmittedReadonlyMedia[] = [];
  let pixels = 0;
  for (const entry of entries) {
    const width = entry.raster.width,
      height = entry.raster.height;
    if (
      !Number.isSafeInteger(width) ||
      !Number.isSafeInteger(height) ||
      width <= 0 ||
      height <= 0 ||
      width * height > 1048576
    )
      return null;
    pixels += width * height;
    if (pixels > 4194304) return null;
    admitted.push(Object.freeze({ entry, width, height }));
  }
  return Object.freeze(admitted);
}
/** One canonical mount owns deferred media descriptors and all duplicate pixel buffers. */
export class ReadonlyWidgetMedia {
  private readonly entries: DeferredReadonlyMedia[] = [];
  private disposed = false;
  private planned = false;
  private preparing = false;
  private readonly current: () => boolean;
  /** Borrow the canonical mount's exact source, resource and geometry lease without becoming another lifetime owner. */
  constructor(current: () => boolean = () => true) {
    this.current = current;
  }
  /** External freshness callbacks may themselves synchronously retire this owner. */
  private available(): boolean {
    return !this.disposed && this.current() && !this.disposed;
  }
  /** Register only prepared pixels; absolute backing chrome already owns native layout. */
  defer(parent: HTMLElement, raster: LoraRaster): void {
    if (this.disposed) throw new ReadonlyMediaRetired();
    if (this.planned || this.preparing) throw new Error('readonly-media-registration-retired');
    this.entries.push({ parent, raster, canvas: null });
  }
  /** Snapshot all mounted prepared descriptors; oversized/invalid batches leave the owner genuinely unplanned for viewport fallback. */
  preflightAll(): ReadonlyMediaPreflight {
    if (this.disposed || this.planned || this.preparing) return { kind: 'retired' };
    this.preparing = true;
    try {
      const entries = admitReadonlyMedia([...this.entries]);
      if (this.disposed) return { kind: 'retired' };
      if (!entries) return { kind: 'deferred' };
      this.planned = true;
      return { kind: 'ready', plan: this.plan(entries) };
    } catch (cause) {
      this.planned = true;
      throw cause;
    } finally {
      this.preparing = false;
    }
  }
  /** Admit requested complete semantic roots through the same dimension/budget owner, retaining one viewport attempt per lifetime. */
  prepare(roots: readonly HTMLElement[]): ReadonlyMediaPlan | null {
    if (this.disposed || this.planned || this.preparing) return null;
    this.planned = true;
    if (!this.available()) return null;
    const selected = this.entries.filter((entry) =>
      roots.some((root) => root.contains(entry.parent)),
    );
    const entries = admitReadonlyMedia(selected);
    if (!entries || !this.available()) return null;
    return this.plan(entries);
  }
  /** Both admission paths delegate one exact hydration attempt and unchanged independent cleanup to this owner. */
  private plan(entries: readonly AdmittedReadonlyMedia[]): ReadonlyMediaPlan {
    let attempt: 'pending' | 'running' | 'complete' | 'failed' = 'pending';
    return Object.freeze({
      empty: entries.length === 0,
      /** One exact attempt owns every returned canvas before invoking any external setter or paint callback. */
      hydrate: (): boolean => {
        if (attempt === 'running' || attempt === 'failed') return false;
        if (attempt === 'complete') return this.available();
        attempt = 'running';
        let constructing: HTMLCanvasElement | null = null;
        try {
          if (!this.available()) {
            attempt = 'failed';
            this.destroy();
            return false;
          }
          for (const { entry, width, height } of entries) {
            if (!this.available() || !entry.parent.isConnected) {
              attempt = 'failed';
              this.destroy();
              return false;
            }
            const canvas = entry.parent.ownerDocument.createElement('canvas');
            constructing = canvas;
            if (this.disposed) {
              constructing = null;
              releaseCanvas(canvas);
              attempt = 'failed';
              return false;
            }
            entry.canvas = canvas;
            /** A setter can retire and then finish writing a formerly owned canvas; clear that untransferred buffer independently. */
            const proceed = (): boolean => {
              if (this.available()) return true;
              constructing = null;
              if (this.disposed) releaseCanvas(canvas);
              else this.destroy();
              attempt = 'failed';
              return false;
            };
            if (!proceed()) return false;
            canvas.className = 'sp-lora-media';
            if (!proceed()) return false;
            canvas.width = width;
            if (!proceed()) return false;
            canvas.height = height;
            if (!proceed()) return false;
            canvas.setAttribute('aria-hidden', 'true');
            if (!proceed()) return false;
            entry.raster.paint(canvas);
            if (!proceed()) return false;
            entry.parent.append(canvas);
            if (!proceed()) return false;
            constructing = null;
          }
          attempt = 'complete';
          return true;
        } catch (primary) {
          attempt = 'failed';
          const causes: unknown[] = [primary];
          if (this.disposed && constructing) {
            try {
              releaseCanvas(constructing);
            } catch (cleanup) {
              causes.push(cleanup);
            }
          }
          try {
            this.destroy();
          } catch (cleanup) {
            causes.push(cleanup);
          }
          if (causes.length > 1)
            throw new AggregateError(causes, 'readonly-media-paint-retirement-failed');
          throw primary;
        }
      },
    });
  }
  /** Retire publication first and release every duplicate buffer while preserving borrowed raster ownership. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    const entries = this.entries.splice(0),
      failures: unknown[] = [];
    for (const entry of entries) {
      const canvas = entry.canvas;
      entry.canvas = null;
      if (!canvas) continue;
      try {
        releaseCanvas(canvas);
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'readonly-media-retirement-failed');
  }
}
