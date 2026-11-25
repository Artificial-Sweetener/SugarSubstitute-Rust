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
/** Lay a Cube chain out in the graph: the one placement every add, move and import uses. */

/** Keep a visible proximity-link corridor between neighbouring Cubes. */
export const MINIMUM_CUBE_CHAIN_GAP = 24;

/** One Cube frame as the graph holds it. */
export interface PlacedFrame {
  readonly id: string;
  readonly position: readonly [number, number];
  readonly size: readonly [number, number];
}

/** One Cube of the requested chain: a Cube in the graph by id, or a new one by its size. */
export type ChainEntry =
  | { readonly id: string; readonly existing: true }
  | { readonly id: string; readonly existing: false; readonly size: readonly [number, number] };

/** A place in the chain: the row it sits on and the space that follows it. */
interface ChainSlot {
  readonly owner: string | null;
  readonly top: number;
  readonly gap: number;
}

/**
 * Place the Cubes of `requested`, in that order, one after the other.
 *
 * The Cubes in the graph (`current`) give the chain its places, read left to right: where the
 * first starts, the row each sits on and the space after each, never under the minimum gap.
 * Reordered Cubes take the places in their new order. A Cube left out of `requested` gives its
 * place up and the chain closes behind it. A new Cube gets a place right after the Cube before
 * it, on the same row with the same space after it, and the Cubes after it move along. With no
 * Cube in the graph, the chain starts at `origin`. An unchanged chain keeps its positions.
 */
export function planChain(
  current: readonly PlacedFrame[],
  requested: readonly ChainEntry[],
  origin: readonly [number, number] = [0, 0],
): Map<string, readonly [number, number]> {
  const byId = new Map(current.map((frame) => [frame.id, frame]));
  const missing = requested.find((entry) => entry.existing && !byId.has(entry.id));
  if (missing) throw new Error(`Cube '${missing.id}' is not in the graph.`);
  const ordered = [...current].sort(
    (left, right) =>
      left.position[0] - right.position[0] ||
      left.position[1] - right.position[1] ||
      (left.id < right.id ? -1 : left.id > right.id ? 1 : 0),
  );
  if (
    requested.length === ordered.length &&
    requested.every((entry, index) => entry.existing && entry.id === ordered[index]?.id)
  )
    return new Map(ordered.map((frame) => [frame.id, frame.position]));
  const kept = new Set(requested.map((entry) => entry.id));
  const slots: ChainSlot[] = ordered.map((frame, index) => {
    const next = ordered[index + 1];
    return {
      owner: frame.id,
      top: frame.position[1],
      gap: next
        ? Math.max(MINIMUM_CUBE_CHAIN_GAP, next.position[0] - frame.position[0] - frame.size[0])
        : MINIMUM_CUBE_CHAIN_GAP,
    };
  });
  const free = slots.filter((slot) => slot.owner !== null && kept.has(slot.owner));
  const sequence: ChainSlot[] = [];
  let next = 0;
  for (const entry of requested) {
    if (entry.existing) {
      const slot = free[next++];
      if (slot) sequence.push(slot);
      continue;
    }
    const before = sequence[sequence.length - 1];
    sequence.push({
      owner: null,
      top: before?.top ?? free[next]?.top ?? origin[1],
      gap: before?.gap ?? MINIMUM_CUBE_CHAIN_GAP,
    });
  }
  const positions = new Map<string, readonly [number, number]>();
  let left = ordered[0]?.position[0] ?? origin[0];
  for (const [index, entry] of requested.entries()) {
    const slot = sequence[index] ?? { owner: null, top: origin[1], gap: MINIMUM_CUBE_CHAIN_GAP };
    positions.set(entry.id, [left, slot.top]);
    const width = entry.existing ? (byId.get(entry.id)?.size[0] ?? 0) : entry.size[0];
    left += Math.max(0, width) + slot.gap;
  }
  return positions;
}
