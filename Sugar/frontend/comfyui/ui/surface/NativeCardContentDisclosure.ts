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
/** Animate changes to native widget membership through the card's shared disclosure owner. */
import { CardDisclosureMotion } from './motion/CardDisclosureMotion.js';
import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';

/** Separate native content invalidation from value updates and initial hydration. */
export class NativeCardContentDisclosure {
  #membership: string | null = null;
  #preparedHeight: number | null = null;

  /** Share the exact mount's motion lifetime with native collapse gestures. */
  constructor(
    private readonly target: HTMLElement,
    private readonly motion: CardDisclosureMotion,
  ) {}

  /** Capture batch geometry only for an upcoming membership transition. */
  prepare(widgets: readonly string[]): void {
    this.#preparedHeight =
      this.#membership !== null && this.#membership !== JSON.stringify(widgets)
        ? this.motion.captureHeight()
        : null;
  }

  /** Render native updates immediately, animating only a changed set of presented widgets. */
  reconcile(widgets: readonly string[], render: () => void): void {
    const membership = JSON.stringify(widgets);
    const changed = this.#membership !== null && this.#membership !== membership;
    const fromHeight = this.#preparedHeight;
    this.#preparedHeight = null;
    this.#membership = membership;
    if (changed)
      this.motion.resize(render, {
        ...(fromHeight !== null ? { fromHeight } : {}),
        settle: () => settleNativeDisclosure(this.target),
      });
    else render();
  }
}
