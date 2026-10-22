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
/** Immutable viewport CSS-pixel paint endpoints retain native glyph dimensions. */
export interface ReorderMotionPoint {
  readonly left: number;
  readonly top: number;
}
/** A complete native chip paint lease supplies position endpoints, never source or caret ranges. */
export interface ReorderMotionTarget {
  readonly id: number;
  readonly kind: 'peer' | 'held';
  readonly from: ReorderMotionPoint;
  readonly to: ReorderMotionPoint;
}
/** One coherent display frame contains only bounded positions and normalized progress. */
export interface ReorderMotionFrame {
  readonly progress: number;
  readonly positions: readonly (ReorderMotionPoint & { readonly id: number })[];
}
/** Native scheduling and the owning presentation's freshness/error boundary remain injected. */
export interface ReorderMotionClockPorts {
  now(): number;
  request(callback: () => void): number;
  cancel(handle: number): void;
  current(): boolean;
  paint(frame: ReorderMotionFrame): void;
  settled(): void;
  failed(cause: unknown): never;
}
/** The donor uses180ms OutQuad peers and OutCubic keyboard-held motion; reduced motion settles immediately. */
export class ReorderMotionClock {
  private readonly ports: ReorderMotionClockPorts;
  private active: {
    started: number;
    readonly targets: readonly ReorderMotionTarget[];
    readonly reduced: boolean;
  } | null = null;
  private handle: number | null = null;
  private frame: ReorderMotionFrame | null = null;
  private disposed = false;
  /** One finite clock owns at most one callback; the caller owns all paint and native authority. */
  constructor(ports: ReorderMotionClockPorts) {
    this.ports = ports;
  }
  /** Retarget from the last actually published positions, without a blank intermediate frame. */
  replace(targets: readonly ReorderMotionTarget[], reduced: boolean): void {
    if (this.disposed) return;
    if (targets.length > 64) throw new TypeError('reorder-motion-target-budget');
    const previous = new Map(this.frame?.positions.map((point) => [point.id, point]));
    const ids = new Set<number>();
    const owned = targets.map((target) => {
      if (
        !Number.isSafeInteger(target.id) ||
        target.id < 0 ||
        ids.has(target.id) ||
        ![target.from.left, target.from.top, target.to.left, target.to.top].every(Number.isFinite)
      )
        throw new TypeError('reorder-motion-geometry');
      ids.add(target.id);
      return Object.freeze({
        ...target,
        from: Object.freeze({ ...(previous.get(target.id) ?? target.from) }),
        to: Object.freeze({ ...target.to }),
      });
    });
    const handle = this.handle;
    this.handle = null;
    const issued = {
      started: 0,
      targets: Object.freeze(owned),
      reduced,
    };
    this.active = issued;
    try {
      if (handle !== null) this.ports.cancel(handle);
      if (this.active !== issued || this.disposed) return;
      const now = this.ports.now();
      if (this.active !== issued || this.disposed) return;
      issued.started = now;
    } catch (cause) {
      if (this.active === issued) this.active = null;
      throw cause;
    }
    this.advance(issued);
  }
  /** Capture only published visual positions; sampling does no layout, parsing or catalog work. */
  positions(): readonly (ReorderMotionPoint & { readonly id: number })[] {
    return this.frame?.positions ?? [];
  }
  /** Every replacement/currentness boundary settles or retires only its own callback. */
  private advance(issued: NonNullable<ReorderMotionClock['active']>): void {
    if (this.active !== issued || this.disposed) return;
    this.handle = null;
    try {
      const current = this.ports.current();
      if (this.active !== issued || this.disposed) return;
      if (!current) {
        this.cancel();
        return;
      }
      const now = this.ports.now();
      if (this.active !== issued || this.disposed) return;
      const elapsed = now - issued.started;
      if (!Number.isFinite(elapsed)) throw new Error('reorder-motion-clock-invalid');
      const progress = issued.reduced ? 1 : Math.max(0, Math.min(1, elapsed / 180));
      const frame = Object.freeze({
        progress,
        positions: Object.freeze(
          issued.targets.map((target) => {
            const eased = 1 - (1 - progress) ** (target.kind === 'held' ? 3 : 2);
            return Object.freeze({
              id: target.id,
              left: target.from.left + (target.to.left - target.from.left) * eased,
              top: target.from.top + (target.to.top - target.from.top) * eased,
            });
          }),
        ),
      });
      this.frame = frame;
      this.ports.paint(frame);
      if (this.active !== issued || this.disposed) return;
      if (progress === 1) {
        this.active = null;
        this.ports.settled();
      } else {
        const handle = this.ports.request(() => this.advance(issued));
        if (this.active === issued && !this.disposed) this.handle = handle;
        else this.ports.cancel(handle);
      }
    } catch (cause) {
      if (this.active !== issued) throw cause;
      try {
        this.cancel();
      } catch (cleanup) {
        this.ports.failed(
          new AggregateError([cause, cleanup], 'reorder-motion-clock-retirement-failed'),
        );
      }
      this.ports.failed(cause);
    }
  }
  /** Retire timing while keeping a frozen last frame available for coherent retarget capture. */
  cancel(): void {
    const handle = this.handle;
    this.handle = null;
    this.active = null;
    if (handle !== null) this.ports.cancel(handle);
  }
  /** Genuine geometry/hold replacement removes old coordinate poses while ordinary target retarget keeps them. */
  reset(): void {
    this.frame = null;
    this.cancel();
  }
  /** A retired presentation releases retained frame data and cannot renew timing. */
  destroy(): void {
    this.disposed = true;
    this.frame = null;
    this.cancel();
  }
}
