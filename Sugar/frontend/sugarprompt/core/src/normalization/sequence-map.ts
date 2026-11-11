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
/** Deterministic semantic source correspondence uses Unicode scalar identities and stable match priority. */
import { requireValue } from '../required.js';
/** One contiguous equal sequence in source/target code-point coordinates. */
interface Match {
  readonly source: number;
  readonly target: number;
  readonly length: number;
}
/** One suffix-equivalence class in a temporary target substring index. */
interface SequenceState {
  readonly next: Map<string, number>;
  length: number;
  link: number;
  firstEnd: number;
}
/** Locate the longest equal block, preferring earliest source then target when lengths tie. */
function longest(
  source: readonly string[],
  target: readonly string[],
  from: number,
  to: number,
  start: number,
  end: number,
): Match {
  const states: SequenceState[] = [{ next: new Map(), length: 0, link: -1, firstEnd: Infinity }];
  let last = 0;
  for (let index = start; index < end; index++) {
    const character = requireValue(target[index], 'sequence-map'),
      current = states.length;
    states.push({
      next: new Map(),
      length: requireValue(states[last], 'sequence-map').length + 1,
      link: 0,
      firstEnd: index,
    });
    let parent = last;
    while (parent >= 0 && !requireValue(states[parent], 'sequence-map').next.has(character)) {
      requireValue(states[parent], 'sequence-map').next.set(character, current);
      parent = requireValue(states[parent], 'sequence-map').link;
    }
    if (parent >= 0) {
      const next = requireValue(
        requireValue(states[parent], 'sequence-map').next.get(character),
        'sequence-map',
      );
      if (
        requireValue(states[parent], 'sequence-map').length + 1 ===
        requireValue(states[next], 'sequence-map').length
      )
        requireValue(states[current], 'sequence-map').link = next;
      else {
        const clone = states.length;
        states.push({
          ...requireValue(states[next], 'sequence-map'),
          next: new Map(requireValue(states[next], 'sequence-map').next),
          length: requireValue(states[parent], 'sequence-map').length + 1,
        });
        while (
          parent >= 0 &&
          requireValue(states[parent], 'sequence-map').next.get(character) === next
        ) {
          requireValue(states[parent], 'sequence-map').next.set(character, clone);
          parent = requireValue(states[parent], 'sequence-map').link;
        }
        requireValue(states[next], 'sequence-map').link = clone;
        requireValue(states[current], 'sequence-map').link = clone;
      }
    }
    last = current;
  }
  for (const state of [...states].sort((a, b) => b.length - a.length)) {
    if (state.link >= 0)
      requireValue(states[state.link], 'sequence-map').firstEnd = Math.min(
        requireValue(states[state.link], 'sequence-map').firstEnd,
        state.firstEnd,
      );
  }
  let best: Match = { source: from, target: start, length: 0 },
    state = 0,
    length = 0;
  for (let index = from; index < to; index++) {
    const character = requireValue(source[index], 'sequence-map');
    while (state && !requireValue(states[state], 'sequence-map').next.has(character)) {
      state = requireValue(states[state], 'sequence-map').link;
      length = requireValue(states[state], 'sequence-map').length;
    }
    const next = requireValue(states[state], 'sequence-map').next.get(character);
    if (next === undefined) {
      state = 0;
      length = 0;
      continue;
    }
    state = next;
    length++;
    if (length > best.length) {
      best = {
        source: index - length + 1,
        target: requireValue(states[state], 'sequence-map').firstEnd - length + 1,
        length,
      };
    }
  }
  return best;
}
/** Match changed local segments without heuristic omission of repeated source characters. */
export function sequenceBoundaryMap(before: string, after: string): readonly number[] {
  const source = [...Array.from(before)],
    target = [...Array.from(after)];
  const queue: [number, number, number, number][] = [[0, source.length, 0, target.length]],
    matches: Match[] = [];
  while (queue.length) {
    const [from, to, start, end] = requireValue(queue.pop(), 'sequence-map');
    const match = longest(source, target, from, to, start, end);
    if (!match.length) continue;
    matches.push(match);
    if (from < match.source && start < match.target)
      queue.push([from, match.source, start, match.target]);
    const nextSource = match.source + match.length,
      nextTarget = match.target + match.length;
    if (nextSource < to && nextTarget < end) queue.push([nextSource, to, nextTarget, end]);
  }
  const blocks: Match[] = [];
  for (const match of matches.sort((a, b) => a.source - b.source || a.target - b.target)) {
    const previous = blocks.at(-1);
    if (
      previous &&
      previous.source + previous.length === match.source &&
      previous.target + previous.length === match.target
    )
      blocks[blocks.length - 1] = {
        ...previous,
        length: previous.length + match.length,
      };
    else blocks.push(match);
  }
  blocks.push({ source: source.length, target: target.length, length: 0 });
  const points: number[] = Array.from({ length: source.length + 1 }, () => 0);
  let from = 0,
    start = 0;
  for (const block of blocks) {
    if (from < block.source || start < block.target) {
      points[from] = start;
      points[block.source] = block.target;
      for (let offset = 1; offset < block.source - from; offset++)
        points[from + offset] = start + Math.min(offset, block.target - start);
    }
    for (let offset = 0; offset <= block.length; offset++)
      points[block.source + offset] = block.target + offset;
    from = block.source + block.length;
    start = block.target + block.length;
  }
  const targetUnits = [0];
  target.forEach((character) =>
    targetUnits.push(requireValue(targetUnits.at(-1), 'sequence-map') + character.length),
  );
  const boundaries: number[] = [];
  let units = 0;
  source.forEach((character, index) => {
    const mapped = requireValue(
      targetUnits[requireValue(points[index], 'sequence-map')],
      'sequence-map',
    );
    boundaries[units] = mapped;
    for (let offset = 1; offset < character.length; offset++)
      boundaries[units + offset] =
        mapped +
        Math.min(
          offset,
          requireValue(
            targetUnits[requireValue(points[index + 1], 'sequence-map')],
            'sequence-map',
          ) - mapped,
        );
    units += character.length;
  });
  boundaries[units] = requireValue(
    targetUnits[requireValue(points[source.length], 'sequence-map')],
    'sequence-map',
  );
  return boundaries;
}
