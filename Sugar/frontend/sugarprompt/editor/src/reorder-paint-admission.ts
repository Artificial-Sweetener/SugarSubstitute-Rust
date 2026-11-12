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
/** One canonical request classifies resource-free fragment construction and retains its exact projected artifact. */
import type { DecorationSet } from '@codemirror/view';
import type { ReorderPaintLine, ReorderWidgetPlan } from './reorder-widget-plan.js';
import { ReorderPaintRetired } from './reorder-widget-block.js';

/** Inventory and allocation budgets remain with LineLayout; this request only owns canonical projection evidence. */
export class ReorderPaintAdmission {
  private readonly byKey = new Map<string, ReorderPaintLine[]>();
  private readonly current: () => boolean;
  /** Capture one immutable plan and its actual native projection, without creating DOM or registering media. */
  constructor(plan: ReorderWidgetPlan, current: () => boolean) {
    this.current = current;
    for (const block of plan.blocks) {
      const groups =
        block.kind === 'chip' ? [block.parts] : block.segments.map((segment) => segment.parts);
      for (const parts of groups)
        for (const part of parts) {
          if (part.kind !== 'line') continue;
          const occurrences = this.byKey.get(part.key) ?? [];
          occurrences.push(part);
          this.byKey.set(part.key, occurrences);
        }
    }
  }
  /** Every occurrence of a missing key must have no marks, widgets or owned row-edge resources before any canonical block is changed. */
  readonly allowMissing = (key: string): boolean => {
    const occurrences = this.byKey.get(key);
    let allowed = !!occurrences;
    for (const part of occurrences ?? []) {
      const decorations = part.decorations;
      if (part.rowPaint || decorations.size !== 0) {
        allowed = false;
        break;
      }
    }
    if (!this.current()) throw new ReorderPaintRetired();
    return allowed;
  };
  /** Canonical construction consumes the exact classified artifact; ordinary cold fragments use the same projection owner. */
  decorations(part: ReorderPaintLine): DecorationSet {
    return part.decorations;
  }
}
