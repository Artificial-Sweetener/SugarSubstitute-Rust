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
/** One gesture owns bounded prepared canonical hard-line paint and actual browser layout reuse. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderRect } from './reorder-geometry.js';
/** Local CSS geometry travels with the exact original painted line, never an estimated width. */
export interface ReorderLineGeometry {
  readonly height: number;
  readonly rects: readonly ReorderRect[];
}
/** A finite line lease retains canonical paint and its original measurement under one complete key. */
export interface ReorderLineLease {
  readonly node: HTMLElement;
  readonly key: string;
  geometry(context: string): ReorderLineGeometry | null;
  measure(context: string, geometry: ReorderLineGeometry): void;
  release(): void;
}
/** Cache identity and resource cleanup belong to the actual painted line, independently of frame borrowing. */
interface LineEntry {
  readonly node: HTMLElement;
  readonly destroy: () => void;
  geometry: {
    readonly context: string;
    readonly value: ReorderLineGeometry;
  } | null;
  borrowed: boolean;
}
/** Idle line paint is bounded; active references are bounded by the current captured line catalogs. */
export class ReorderLineLayout {
  private readonly capacity: number;
  private readonly entries = new Map<string, LineEntry[]>();
  private readonly idle = new Map<LineEntry, string>();
  private disposed = false;
  private reconciliationDepth = 0;
  private existingOnly = false;
  private creationBudget: Map<string, number> | null = null;
  /** A cold capture retains at most 256 idle lines, independent of target visit count. */
  constructor(capacity = 256) {
    if (!Number.isSafeInteger(capacity) || capacity < 1)
      throw new Error('reorder-line-cache-capacity');
    this.capacity = capacity;
  }
  /** Synchronous frame replacement may release and reborrow actual paint before the outermost scope restores the idle bound; nested scopes share that same finite lifetime. */
  reconcile(action: () => void): void {
    if (this.disposed) throw new Error('reorder-line-cache-retired');
    this.reconciliationDepth++;
    const failures: unknown[] = [];
    try {
      action();
    } catch (cause) {
      failures.push(cause);
    }
    this.reconciliationDepth--;
    if (!this.reconciliationDepth) {
      try {
        this.trim();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder line reconciliation failed');
  }
  /** Preflight complete multiplicities before mutation; the canonical caller may admit only missing resource-free fragments, while this owner reserves every exact creation occurrence. */
  reconcilePrepared(
    keys: readonly string[],
    classifyMissing: (key: string) => boolean,
    action: () => void,
  ): boolean {
    if (this.disposed || this.existingOnly) return false;
    const counts = new Map<string, number>(),
      missing = new Map<string, number>();
    for (const key of keys) counts.set(key, (counts.get(key) ?? 0) + 1);
    for (const [key, count] of counts) {
      const needed = count - (this.entries.get(key)?.length ?? 0);
      if (needed > 0) missing.set(key, needed);
    }
    this.existingOnly = true;
    try {
      // No factory is admitted during classification, including reentrant reconciliation.
      for (const key of missing.keys()) {
        if (!classifyMissing(key) || this.disposed) return false;
      }
      this.creationBudget = missing;
      this.reconcile(action);
      return true;
    } finally {
      this.creationBudget = null;
      this.existingOnly = false;
    }
  }
  /** Revisit borrows the actual idle DOM/layout; concurrent base and accepted lines retain independent paint. */
  borrow(
    key: string,
    create: () => {
      readonly node: HTMLElement;
      readonly destroy: () => void;
    },
  ): ReorderLineLease {
    if (this.disposed) throw new Error('reorder-line-cache-retired');
    let entries = this.entries.get(key) ?? [];
    let entry = entries.find((value) => !value.borrowed);
    if (!entry) {
      if (this.existingOnly) {
        const remaining = this.creationBudget?.get(key) ?? 0;
        if (remaining === 0) throw new Error('reorder-line-existing-paint-missing');
        // Consume before external construction so nested borrowing cannot reuse this occurrence.
        requireValue(this.creationBudget, 'reorder-line-layout').set(key, remaining - 1);
      }
      const created = create();
      if (this.disposed) {
        const failures: unknown[] = [new Error('reorder-line-cache-retired')];
        try {
          created.destroy();
        } catch (cause) {
          failures.push(cause);
        }
        try {
          created.node.remove();
        } catch (cause) {
          failures.push(cause);
        }
        if (failures.length === 1) throw failures[0];
        throw new AggregateError(failures, 'Reorder line creation retirement failed');
      }
      // Canonical creation can reenter this owner; retain the actual bucket installed by that nested borrow.
      entries = this.entries.get(key) ?? [];
      entry = {
        ...created,
        geometry: entries.find((value) => value.geometry)?.geometry ?? null,
        borrowed: false,
      };
      entries.push(entry);
    }
    this.entries.delete(key);
    this.entries.set(key, entries);
    entry.borrowed = true;
    this.idle.delete(entry);
    const captured = entry;
    let released = false;
    return Object.freeze({
      node: captured.node,
      key,
      geometry: (context: string): ReorderLineGeometry | null =>
        !released && !this.disposed && captured.geometry?.context === context
          ? captured.geometry.value
          : null,
      measure: (context: string, geometry: ReorderLineGeometry): void => {
        if (released || this.disposed) throw new Error('reorder-line-measurement-retired');
        if (
          !(geometry.height > 0) ||
          !Number.isFinite(geometry.height) ||
          geometry.rects.some(
            (rect) => ![rect.left, rect.right, rect.top, rect.bottom].every(Number.isFinite),
          )
        )
          throw new Error('reorder-line-measurement-invalid');
        captured.geometry = Object.freeze({
          context,
          value: Object.freeze({
            height: geometry.height,
            rects: Object.freeze(geometry.rects.map((rect) => Object.freeze({ ...rect }))),
          }),
        });
      },
      release: (): void => {
        if (released) return;
        released = true;
        if (this.disposed) return;
        captured.borrowed = false;
        this.idle.set(captured, key);
        const failures: unknown[] = [];
        try {
          captured.node.remove();
        } catch (cause) {
          failures.push(cause);
        }
        try {
          if (!this.reconciliationDepth) this.trim();
        } catch (cause) {
          failures.push(cause);
        }
        if (failures.length === 1) throw failures[0];
        if (failures.length) throw new AggregateError(failures, 'Reorder line release failed');
      },
    });
  }
  /** Evict least-recent idle actual paint, preserving active frame leases and all unexpected cleanup causes. */
  private trim(): void {
    const failures: unknown[] = [];
    while (this.idle.size > this.capacity) {
      const [entry, key] = requireValue(this.idle.entries().next().value, 'reorder-line-layout');
      this.idle.delete(entry);
      const entries = requireValue(this.entries.get(key), 'reorder-line-layout');
      entries.splice(entries.indexOf(entry), 1);
      if (!entries.length) this.entries.delete(key);
      try {
        entry.destroy();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length) throw new AggregateError(failures, 'Reorder line eviction failed');
  }
  /** Gesture/context retirement fences borrowing before independently retiring every actual paint lifetime. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    const entries = [...this.entries.values()].flat();
    this.entries.clear();
    this.idle.clear();
    const failures: unknown[] = [];
    for (const entry of entries) {
      try {
        entry.destroy();
      } catch (cause) {
        failures.push(cause);
      }
      try {
        entry.node.remove();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder line retirement failed');
  }
}
