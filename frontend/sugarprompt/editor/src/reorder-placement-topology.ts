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
/** One prepared physical-slot topology adapts genuine measured fragments to Core insertion identities. */
import { requireValue } from '../../core/src/required.js';
import { type ReorderDropTarget } from '../../core/src/index.js';
import type { ReorderMeasuredGeometry, ReorderRect } from './reorder-geometry.js';
/** A logical target may have several actual wrapped occurrences, each in one physical lane. */
export interface ReorderOccurrence {
  readonly target: ReorderDropTarget;
  readonly x: number;
  readonly y: number;
  readonly lane: number;
}
/** Exact Core target equality keeps distinct blank ordinals and regional partitions. */
export function sameReorderTarget(a: ReorderDropTarget, b: ReorderDropTarget): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === 'row' && b.kind === 'row') return a.row === b.row && a.insertion === b.insertion;
  if (a.kind === 'gap' && b.kind === 'gap') return a.gap === b.gap && a.blank === b.blank;
  return a.kind === 'partition' && b.kind === 'partition' && a.partition === b.partition;
}
/** Actual native physical-line and caret operands are captured once by the mounted geometry owner. */
export interface ReorderPhysicalFragment {
  readonly rect: ReorderRect;
  readonly line: {
    readonly top: number;
    readonly bottom: number;
    readonly identity: number;
  };
  readonly leading: number;
  readonly trailing: number;
}
/** A prepared insertion occurrence owns no source or layout estimation. */
export interface ReorderPlacementSlot extends ReorderOccurrence {
  readonly anchor: number;
  readonly hit: {
    readonly left: number;
    readonly right: number;
    readonly top: number;
    readonly bottom: number;
  };
  readonly line: {
    readonly top: number;
    readonly bottom: number;
  };
}
/** One finite current measured output is shared by pointer bands and directional occurrence traversal. */
export interface ReorderPlacementTopology {
  readonly slots: readonly ReorderPlacementSlot[];
}
/** Compile bounded native fragments once; source order remains solely owned by the supplied Core layout. */
export function prepareReorderPlacementTopology(
  layout: import('../../core/src/index.js').ReorderLayout,
  fragments: readonly ReorderPhysicalFragment[],
  viewport: ReorderMeasuredGeometry['viewport'],
): ReorderPlacementTopology | null {
  if (
    fragments.length > 8192 ||
    ![viewport.left, viewport.right, viewport.top, viewport.bottom].every(Number.isFinite) ||
    viewport.right <= viewport.left ||
    viewport.bottom <= viewport.top
  )
    return null;
  const rows = new Map(
    layout.rows.flatMap((row) =>
      row.chips.map((chip, insertion) => [chip, { row: row.index, insertion }] as const),
    ),
  );
  const lanes = new Map<
    string,
    {
      readonly first: number;
      readonly fragments: ReorderPhysicalFragment[];
      readonly identities: Map<string, number>;
    }
  >();
  const chipLines = new Set(
    fragments
      .filter((fragment) => fragment.rect.kind === 'chip' && rows.has(fragment.rect.index))
      .map((fragment) => fragment.line.identity),
  );
  for (const [ordinal, fragment] of fragments.entries()) {
    const { rect, line } = fragment;
    if (
      ![
        rect.left,
        rect.right,
        rect.top,
        rect.bottom,
        line.top,
        line.bottom,
        fragment.leading,
        fragment.trailing,
      ].every(Number.isFinite) ||
      rect.right <= rect.left ||
      rect.bottom <= rect.top ||
      line.bottom <= line.top ||
      !Number.isSafeInteger(line.identity)
    )
      return null;
    const row = rect.kind === 'chip' ? rows.get(rect.index) : undefined;
    if (rect.kind === 'chip' && !row) return null;
    if (
      rect.kind === 'gap' &&
      (rect.blank === undefined ||
        layout.gaps[rect.index]?.blankOffsets[rect.blank] === undefined ||
        chipLines.has(line.identity))
    )
      continue;
    const key = `${row ? `row:${row.row}` : `${rect.kind}:${rect.index}:${rect.blank ?? ''}`}:${line.identity}`;
    let lane = lanes.get(key);
    if (!lane) {
      lane = { first: ordinal, fragments: [], identities: new Map() };
      lanes.set(key, lane);
    }
    const fragmentKey = `${rect.kind}:${rect.index}:${rect.blank ?? ''}`;
    const existing = lane.identities.get(fragmentKey);
    if (existing === undefined) {
      lane.identities.set(fragmentKey, lane.fragments.length);
      lane.fragments.push(fragment);
    } else {
      const previous = requireValue(lane.fragments[existing], 'reorder-placement-topology');
      lane.fragments[existing] = {
        line: previous.line,
        leading: previous.leading,
        trailing: fragment.trailing,
        rect: {
          ...previous.rect,
          left: Math.min(previous.rect.left, rect.left),
          right: Math.max(previous.rect.right, rect.right),
          top: Math.min(previous.rect.top, rect.top),
          bottom: Math.max(previous.rect.bottom, rect.bottom),
        },
      };
    }
  }
  const ordered = [...lanes.values()].sort(
    (a, b) =>
      requireValue(a.fragments[0], 'reorder-placement-topology').line.top -
        requireValue(b.fragments[0], 'reorder-placement-topology').line.top || a.first - b.first,
  );
  const slots: ReorderPlacementSlot[] = [];
  for (const [lane, items] of ordered.entries()) {
    const top = Math.min(
        ...items.fragments.map((fragment) => Math.min(fragment.line.top, fragment.rect.top)),
      ),
      bottom = Math.max(
        ...items.fragments.map((fragment) => Math.max(fragment.line.bottom, fragment.rect.bottom)),
      );
    const line = Object.freeze({ top, bottom });
    const chips = items.fragments
      .filter((fragment) => fragment.rect.kind === 'chip')
      .sort((a, b) => a.rect.left + a.rect.right - (b.rect.left + b.rect.right));
    const add = (
      target: ReorderDropTarget,
      left: number,
      right: number,
      anchor: number,
      y: number,
    ): void => {
      const hitRight = Math.max(left + 8, right);
      slots.push(
        Object.freeze({
          target: Object.freeze(target),
          lane,
          x: (left + hitRight) / 2,
          y,
          anchor,
          line,
          hit: Object.freeze({
            left: Math.max(viewport.left, left),
            right: Math.min(viewport.right, hitRight),
            top: Math.max(viewport.top, top),
            bottom: Math.min(viewport.bottom, bottom),
          }),
        }),
      );
    };
    for (let at = 0; chips.length && at <= chips.length; at++) {
      const before = chips[at - 1],
        after = chips[at],
        fragment = after ?? requireValue(before, 'reorder-placement-topology'),
        position = requireValue(rows.get(fragment.rect.index), 'reorder-placement-topology');
      add(
        {
          kind: 'row',
          row: position.row,
          insertion: position.insertion + (after ? 0 : 1),
        },
        before ? (before.rect.left + before.rect.right) / 2 : viewport.left,
        after ? (after.rect.left + after.rect.right) / 2 : viewport.right,
        after ? after.leading : requireValue(before, 'reorder-placement-topology').trailing,
        (fragment.rect.top + fragment.rect.bottom) / 2,
      );
    }
    for (const fragment of items.fragments) {
      const { rect } = fragment;
      if (rect.kind === 'chip') continue;
      add(
        rect.kind === 'partition'
          ? { kind: 'partition', partition: rect.index }
          : {
              kind: 'gap',
              gap: rect.index,
              blank: requireValue(rect.blank, 'reorder-placement-topology'),
            },
        viewport.left,
        viewport.right,
        fragment.leading,
        (rect.top + rect.bottom) / 2,
      );
    }
  }
  return Object.freeze({ slots: Object.freeze(slots) });
}
/** Containment retains the exact current physical placement; fallback chooses a vertical line then actual insertion-anchor X. */
export function resolveReorderPlacement(
  topology: ReorderPlacementTopology | null,
  point: {
    readonly x: number;
    readonly y: number;
  },
  current: ReorderDropTarget | null,
  previous: ReorderPlacementSlot | null,
): ReorderPlacementSlot | null {
  if (!topology || ![point.x, point.y].every(Number.isFinite)) return null;
  const contained = topology.slots.filter(
    (slot) =>
      slot.hit.right > slot.hit.left &&
      slot.hit.bottom > slot.hit.top &&
      point.x >= slot.hit.left &&
      point.x <= slot.hit.right &&
      point.y >= slot.hit.top &&
      point.y <= slot.hit.bottom,
  );
  if (contained.length) {
    if (
      previous &&
      current &&
      sameReorderTarget(previous.target, current) &&
      contained.includes(previous)
    )
      return previous;
    const active = current && contained.find((slot) => sameReorderTarget(slot.target, current));
    if (active) return active;
    return contained.reduce((best, slot) =>
      Math.abs(slot.anchor - point.x) < Math.abs(best.anchor - point.x) ? slot : best,
    );
  }
  let line: ReorderPlacementSlot | null = null,
    distance = Infinity;
  for (const slot of topology.slots) {
    if (slot.hit.right <= slot.hit.left || slot.hit.bottom <= slot.hit.top) continue;
    const dy = Math.max(slot.hit.top - point.y, 0, point.y - slot.hit.bottom);
    if (dy < distance) {
      distance = dy;
      line = slot;
    }
  }
  if (!line) return null;
  const candidates = topology.slots.filter((slot) => slot.lane === line.lane);
  return candidates.reduce((best, slot) =>
    Math.abs(slot.anchor - point.x) < Math.abs(best.anchor - point.x) ? slot : best,
  );
}
