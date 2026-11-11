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
/** Pure token-specific wheel authorization; native geometry and source lifetimes remain separate owners. */
import type { WheelMode } from './wheel-mode.js';

/** Viewport CSS pixels establish stable hover independently of logical-document UTF-16. */
export interface WheelPoint {
  readonly x: number;
  readonly y: number;
}

/** A finite view-owned policy stores opaque token identities, points and monotonic timestamps only. */
export class WheelIntent {
  private candidate: {
    readonly target: symbol;
    readonly point: WheelPoint;
    readonly startedAt: number;
  } | null = null;
  private armed: symbol | null = null;
  private active: symbol | null = null;
  private latched: symbol | null = null;
  private lastWheel = 0;

  /** A real pointer move changes targets or restarts dwell beyond six CSS pixels. */
  move(target: symbol, point: WheelPoint, now: number): void {
    if (this.latched !== target) this.latched = null;
    if (this.armed === target) return;
    if (
      this.candidate?.target !== target ||
      Math.hypot(point.x - this.candidate.point.x, point.y - this.candidate.point.y) > 6
    ) {
      this.armed = null;
      this.candidate = { target, point: { ...point }, startedAt: now };
    }
  }

  /** Click activation authorizes exactly one token in focus-required mode. */
  activate(target: symbol): void {
    this.active = target;
    if (this.latched !== target) this.latched = null;
  }

  /** The native lifetime schedules only the remaining dwell deadline; absent candidates own no timer. */
  remainingDwell(target: symbol, now: number): number | null {
    return this.candidate?.target === target && this.armed !== target
      ? Math.max(0, 400 - (now - this.candidate.startedAt))
      : null;
  }

  /** Observe readiness without resetting dwell; an idle burst expires after 250 milliseconds. */
  ready(target: symbol, mode: WheelMode, now: number): boolean {
    if (this.latched !== null && now - this.lastWheel >= 250) this.latched = null;
    if (mode === 'focus_required') return this.active === target;
    if (this.candidate?.target === target && now - this.candidate.startedAt >= 400)
      this.armed = target;
    return this.armed === target;
  }

  /** A premature tick remains ordinary scrolling and restarts dwell; authorized bursts retain the same token. */
  allow(target: symbol, mode: WheelMode, now: number): boolean {
    if (this.ready(target, mode, now) || this.latched === target) {
      this.latched = target;
      this.lastWheel = now;
      return true;
    }
    if (this.candidate?.target === target) this.candidate = { ...this.candidate, startedAt: now };
    return false;
  }

  /** Pointer departure retires hover and a burst while preserving deliberate token activation. */
  leave(): void {
    this.candidate = null;
    this.armed = null;
    this.latched = null;
  }

  /** Native source/context/focus replacement and teardown release every identity. */
  clear(): void {
    this.leave();
    this.active = null;
  }
}
