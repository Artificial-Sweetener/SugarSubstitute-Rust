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
/** One Scene's actual readonly writes retain native inline fallback and admitted complete atomic carriers under the same Slot reservation. */
import type { ReorderMotionFrame } from './reorder-motion-clock.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
import type { ReorderMotionPeerCarriers } from './reorder-motion-peer-carriers.js';

/** Paint owns no source, scheduling or lease authority; every native write borrows the enclosing Slot's exact publication predicate. */
export class ReorderScenePaint {
  private readonly chips: ReorderMotionScene['chips'];
  private readonly carriers: ReorderMotionPeerCarriers | null;
  /** Capture the exact immutable Scene output and its optional already revalidated complete copies. */
  constructor(chips: ReorderMotionScene['chips'], carriers: ReorderMotionPeerCarriers | null) {
    this.chips = chips;
    this.carriers = carriers;
  }

  /** Reveal complete copies first, while unsupported peers retain the original native inline styling. */
  reveal(current: () => boolean): void {
    if (this.carriers && !this.carriers.reveal(current)) {
      if (!current()) return;
      throw new Error('reorder-peer-carrier-reveal-unavailable');
    }
    for (const chip of this.chips)
      for (const root of this.carriers?.handles(chip.id) ? [] : chip.roots) {
        if (!current()) return;
        root.node.style.visibility = 'visible';
        if (!current()) return;
        root.node.style.position = 'relative';
        if (!current()) return;
        root.node.dataset.reorderMotionChip = String(chip.id);
      }
  }

  /** A complete carrier changes only transform; generic roots preserve the existing independently scaled relative offsets. */
  paint(frame: ReorderMotionFrame, current: () => boolean): void {
    for (const point of frame.positions) {
      const chip = this.chips.find((entry) => entry.id === point.id);
      if (!chip) continue;
      if (this.carriers?.paint(point.id, point.left, point.top, current)) continue;
      for (const root of chip.roots) {
        if (!current()) return;
        root.node.style.left = `${(point.left - chip.left) / root.scaleX}px`;
        if (!current()) return;
        root.node.style.top = `${(point.top - chip.top) / root.scaleY}px`;
      }
    }
  }

  /** Retirement closes every carrier and restores each independently attempted generic declaration before the Slot can lend this output. */
  close(current: () => boolean): readonly unknown[] {
    const failures: unknown[] = [];
    try {
      this.carriers?.destroy();
    } catch (cause) {
      failures.push(cause);
    }
    for (const chip of this.chips)
      for (const root of this.carriers?.handles(chip.id) ? [] : chip.roots)
        for (const reset of [
          () => {
            root.node.style.visibility = '';
          },
          () => {
            root.node.style.position = '';
          },
          () => {
            root.node.style.left = '';
          },
          () => {
            root.node.style.top = '';
          },
          () => {
            delete root.node.dataset.reorderMotionChip;
          },
        ])
          try {
            if (current()) reset();
          } catch (cause) {
            failures.push(cause);
          }
    return failures;
  }
}
