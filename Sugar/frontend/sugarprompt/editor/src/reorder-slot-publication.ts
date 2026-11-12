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
/** One owned shell's physical rectangle publication reserves native setter stacks without owning a Scene, lease or resource cache. */
import type { ReorderMarkerRect } from './reorder-geometry.js';

/** Native clipping and root preparation settle before the retaining Slot may lend this same physical shell again. */
export class ReorderSlotPublication {
  private readonly node: HTMLElement;
  private readonly parent: HTMLElement;
  private writes = 0;
  private disposed = false;
  /** Capture one finite physical shell; acquiring it does not publish pixels. */
  constructor(node: HTMLElement, parent: HTMLElement) {
    this.node = node;
    this.parent = parent;
  }
  /** An in-flight native delegate is non-lendable even when cancellation has already revoked its logical lease. */
  get busy(): boolean {
    return this.writes !== 0;
  }
  /** Close construction before any connected output or media callback can publish it. */
  initialize(admitted: () => boolean): void {
    const failures = this.close(admitted, false, () => {
      if (!this.disposed && admitted() && !this.disposed)
        this.node.style.setProperty('position', 'absolute', 'important');
      return [];
    });
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Passive slot initial closure failed');
  }
  /** Reserve root/frame/endpoint setters; optional clipping commits only after complete admission, while stale delegates close only this captured shell. */
  write(
    owned: () => boolean,
    admitted: () => boolean,
    action: (current: () => boolean) => void,
    clip: ReorderMarkerRect | null | undefined,
    failed: () => void,
  ): void {
    if (this.disposed || this.busy) return;
    const current = (): boolean =>
      !this.disposed &&
      owned() &&
      !this.disposed &&
      this.node.isConnected &&
      this.node.parentElement === this.parent &&
      !this.disposed;
    const failures: unknown[] = [];
    this.writes++;
    try {
      try {
        if (current() && admitted() && current()) action(current);
        if (clip !== undefined && current() && admitted() && current())
          this.node.style.setProperty(
            'clip',
            clip
              ? `rect(${clip.top}px, ${clip.right}px, ${clip.bottom}px, ${clip.left}px)`
              : 'auto',
            'important',
          );
      } catch (cause) {
        failures.push(cause);
      }
      try {
        if (failures.length || !current() || !admitted() || !current())
          failures.push(...this.empty(() => true));
      } catch (cause) {
        failures.push(cause);
      }
    } finally {
      this.writes--;
    }
    if (!failures.length) return;
    try {
      failed();
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    throw new AggregateError(failures, 'Passive slot publication failed');
  }
  /** Independently close pixels, publication attributes and roots; nested cancellation never clears an outer native-write reservation. */
  close(
    admitted: () => boolean,
    partial: boolean,
    resetRoots: () => readonly unknown[],
  ): readonly unknown[] {
    const failures: unknown[] = [];
    this.writes++;
    try {
      failures.push(...this.empty(() => !this.disposed && admitted() && !this.disposed));
      try {
        if (partial && !this.disposed && admitted() && !this.disposed)
          this.node.style.visibility = 'hidden';
      } catch (cause) {
        failures.push(cause);
      }
      try {
        failures.push(...resetRoots());
      } catch (cause) {
        failures.push(cause);
      }
    } finally {
      this.writes--;
    }
    return failures;
  }
  /** Captured stale continuations clear only this immutable physical node, independently attempting every publication reset. */
  private empty(admitted: () => boolean): readonly unknown[] {
    const failures: unknown[] = [];
    for (const reset of [
      () => this.node.style.setProperty('clip', 'rect(0px, 0px, 0px, 0px)', 'important'),
      () => {
        delete this.node.dataset.reorderMotionProgress;
      },
      () => {
        delete this.node.dataset.reorderMotionEndpoint;
      },
    ])
      try {
        if (admitted()) reset();
      } catch (cause) {
        failures.push(cause);
      }
    return failures;
  }
  /** Revocation leaves any captured late setter responsible only for this old shell's closure. */
  retire(): void {
    this.disposed = true;
  }
}
