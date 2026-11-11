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
/** Bounded viewport-pixel rounded-bubble union geometry owns no source ranges, projection or native editor state. */
import { requireValue } from '../../core/src/required.js';
/** Already inflated/clipped viewport-CSS-pixel bubbles come from the sole native landing adaptation. */
export interface ReorderLandingBubble {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}
/** Row comparisons and union intersection/stitch work each have 32,768 units; sorting is separately bounded by native fragment admission. */
export const landingStageWorkLimit = 32768;
interface Point {
  readonly x: number;
  readonly y: number;
}
interface Bubble {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
  readonly radius: number;
}
type Edge =
  | {
      readonly kind: 'line';
      readonly from: Point;
      readonly to: Point;
    }
  | {
      readonly kind: 'arc';
      readonly center: Point;
      readonly radius: number;
      readonly start: number;
      readonly end: number;
    };
/** Complete contour admission is finite; unavailable paint never rejects an otherwise genuine gesture or source edit. */
export type ReorderLandingContour =
  | {
      readonly status: 'ready';
      readonly path: string;
      readonly left: number;
      readonly top: number;
      readonly width: number;
      readonly height: number;
    }
  | {
      readonly status: 'unavailable';
      readonly reason:
        | 'fragment-budget'
        | 'invalid-geometry'
        | 'work-budget'
        | 'incomplete-contour';
    };
const epsilon = 1e-7,
  tau = Math.PI * 2;
/** Stable endpoint keys reconcile analytical intersections without changing native glyph or hit coordinates. */
function scalar(value: number): number {
  return Math.round(value * 1e6) / 1e6;
}
function key(point: Point): string {
  return `${scalar(point.x)},${scalar(point.y)}`;
}
function at(edge: Edge, progress: number): Point {
  if (edge.kind === 'line')
    return {
      x: edge.from.x + (edge.to.x - edge.from.x) * progress,
      y: edge.from.y + (edge.to.y - edge.from.y) * progress,
    };
  const angle = edge.start + (edge.end - edge.start) * progress;
  return {
    x: edge.center.x + Math.cos(angle) * edge.radius,
    y: edge.center.y + Math.sin(angle) * edge.radius,
  };
}
/** Directed tangents distinguish shared interior edges from duplicate exterior boundaries. */
function tangent(edge: Edge, progress: number): Point {
  if (edge.kind === 'line') {
    const x = edge.to.x - edge.from.x,
      y = edge.to.y - edge.from.y,
      length = Math.hypot(x, y);
    return { x: x / length, y: y / length };
  }
  const angle = edge.start + (edge.end - edge.start) * progress;
  return { x: -Math.sin(angle), y: Math.cos(angle) };
}
/** Clockwise primitives are the actual rounded rectangle boundary, including width/height-limited radii. */
function edges(bubble: Bubble): readonly Edge[] {
  const { left: l, right: r, top: t, bottom: b, radius: q } = bubble;
  return [
    { kind: 'line', from: { x: l + q, y: t }, to: { x: r - q, y: t } },
    {
      kind: 'arc',
      center: { x: r - q, y: t + q },
      radius: q,
      start: -Math.PI / 2,
      end: 0,
    },
    { kind: 'line', from: { x: r, y: t + q }, to: { x: r, y: b - q } },
    {
      kind: 'arc',
      center: { x: r - q, y: b - q },
      radius: q,
      start: 0,
      end: Math.PI / 2,
    },
    { kind: 'line', from: { x: r - q, y: b }, to: { x: l + q, y: b } },
    {
      kind: 'arc',
      center: { x: l + q, y: b - q },
      radius: q,
      start: Math.PI / 2,
      end: Math.PI,
    },
    { kind: 'line', from: { x: l, y: b - q }, to: { x: l, y: t + q } },
    {
      kind: 'arc',
      center: { x: l + q, y: t + q },
      radius: q,
      start: Math.PI,
      end: Math.PI * 1.5,
    },
  ];
}
/** Return a boundary parameter only for an actual finite segment or quarter arc. */
function parameter(edge: Edge, point: Point): number | null {
  let value: number;
  if (edge.kind === 'line') {
    const dx = edge.to.x - edge.from.x,
      dy = edge.to.y - edge.from.y,
      length = dx * dx + dy * dy;
    if (length <= epsilon) return null;
    value = ((point.x - edge.from.x) * dx + (point.y - edge.from.y) * dy) / length;
    const projected = at(edge, value);
    if (Math.hypot(projected.x - point.x, projected.y - point.y) > epsilon) return null;
  } else {
    if (
      Math.abs(Math.hypot(point.x - edge.center.x, point.y - edge.center.y) - edge.radius) > epsilon
    )
      return null;
    let angle = Math.atan2(point.y - edge.center.y, point.x - edge.center.x);
    while (angle < edge.start - epsilon) angle += tau;
    value = (angle - edge.start) / (edge.end - edge.start);
  }
  return value >= -epsilon && value <= 1 + epsilon ? Math.max(0, Math.min(1, value)) : null;
}
/** Analytic circle/segment intersections retain fractional CSS coordinates without estimated layout. */
function lineCircle(
  line: Extract<
    Edge,
    {
      kind: 'line';
    }
  >,
  arc: Extract<
    Edge,
    {
      kind: 'arc';
    }
  >,
): readonly Point[] {
  const dx = line.to.x - line.from.x,
    dy = line.to.y - line.from.y,
    fx = line.from.x - arc.center.x,
    fy = line.from.y - arc.center.y,
    a = dx * dx + dy * dy,
    b = 2 * (fx * dx + fy * dy),
    c = fx * fx + fy * fy - arc.radius * arc.radius,
    discriminant = b * b - 4 * a * c;
  if (a <= epsilon || discriminant < -epsilon) return [];
  return [
    (-b - Math.sqrt(Math.max(0, discriminant))) / (2 * a),
    (-b + Math.sqrt(Math.max(0, discriminant))) / (2 * a),
  ]
    .filter((value) => value >= -epsilon && value <= 1 + epsilon)
    .map((value) => at(line, value));
}
/** Only intersections admitted by both original primitives can divide their exposed intervals. */
function intersections(first: Edge, second: Edge): readonly Point[] {
  let points: readonly Point[];
  if (first.kind === 'line' && second.kind === 'line') {
    const ax = first.to.x - first.from.x,
      ay = first.to.y - first.from.y,
      bx = second.to.x - second.from.x,
      by = second.to.y - second.from.y,
      cross = ax * by - ay * bx;
    if (Math.abs(cross) <= epsilon) points = [second.from, second.to];
    else {
      const progress =
        ((second.from.x - first.from.x) * by - (second.from.y - first.from.y) * bx) / cross;
      points = [at(first, progress)];
    }
  } else if (first.kind === 'line' && second.kind === 'arc') points = lineCircle(first, second);
  else if (first.kind === 'arc' && second.kind === 'line') points = lineCircle(second, first);
  else if (first.kind === 'arc' && second.kind === 'arc') {
    const dx = second.center.x - first.center.x,
      dy = second.center.y - first.center.y,
      distance = Math.hypot(dx, dy);
    if (distance <= epsilon)
      points =
        Math.abs(first.radius - second.radius) <= epsilon ? [at(second, 0), at(second, 1)] : [];
    else if (
      distance > first.radius + second.radius + epsilon ||
      distance < Math.abs(first.radius - second.radius) - epsilon
    )
      points = [];
    else {
      const along =
          (first.radius * first.radius - second.radius * second.radius + distance * distance) /
          (2 * distance),
        height = Math.sqrt(Math.max(0, first.radius * first.radius - along * along)),
        x = first.center.x + (along * dx) / distance,
        y = first.center.y + (along * dy) / distance;
      points = [
        { x: x - (height * dy) / distance, y: y + (height * dx) / distance },
        { x: x + (height * dy) / distance, y: y - (height * dx) / distance },
      ];
    }
  } else points = [];
  return points.filter(
    (point) => parameter(first, point) !== null && parameter(second, point) !== null,
  );
}
/** Signed inset distinguishes genuine interior from coincident exterior boundaries. */
function inset(bubble: Bubble, point: Point): number {
  if (
    point.x < bubble.left - epsilon ||
    point.x > bubble.right + epsilon ||
    point.y < bubble.top - epsilon ||
    point.y > bubble.bottom + epsilon
  )
    return -Infinity;
  const x = Math.max(bubble.left + bubble.radius, Math.min(bubble.right - bubble.radius, point.x)),
    y = Math.max(bubble.top + bubble.radius, Math.min(bubble.bottom - bubble.radius, point.y));
  return bubble.radius - Math.hypot(point.x - x, point.y - y);
}
/** One union gets at most 256 bubbles, 32,768 comparisons/intersections and 8,192 exterior pieces. */
export function prepareReorderLandingContour(
  fragments: readonly ReorderLandingBubble[],
): ReorderLandingContour {
  if (!fragments.length || fragments.length > 256)
    return { status: 'unavailable', reason: 'fragment-budget' };
  const bubbles = fragments.map((rect) => ({
    left: rect.left,
    right: rect.right,
    top: rect.top,
    bottom: rect.bottom,
    radius: Math.min(9, (rect.right - rect.left) / 2, (rect.bottom - rect.top) / 2),
  }));
  if (
    bubbles.some(
      (bubble) =>
        ![
          bubble.left,
          bubble.right,
          bubble.top,
          bubble.bottom,
          bubble.radius,
          bubble.right - bubble.left,
          bubble.bottom - bubble.top,
        ].every(
          (value) => Number.isFinite(value) && Number.isSafeInteger(Math.round(value * 1e6)),
        ) ||
        bubble.right <= bubble.left ||
        bubble.bottom <= bubble.top ||
        bubble.radius <= 0,
    )
  )
    return { status: 'unavailable', reason: 'invalid-geometry' };
  const boundaries = bubbles.map(edges),
    pieces: {
      readonly from: Point;
      readonly to: Point;
      readonly command: string;
    }[] = [];
  let work = 0;
  for (const [index, bubble] of bubbles.entries())
    for (const edge of requireValue(boundaries[index], 'reorder-landing-geometry')) {
      const divisions = [0, 1],
        overlaps: number[] = [];
      for (const [otherIndex, other] of bubbles.entries()) {
        if (++work > landingStageWorkLimit) return { status: 'unavailable', reason: 'work-budget' };
        if (
          otherIndex === index ||
          other.left > bubble.right ||
          other.right < bubble.left ||
          other.top > bubble.bottom ||
          other.bottom < bubble.top
        )
          continue;
        overlaps.push(otherIndex);
        for (const boundary of requireValue(boundaries[otherIndex], 'reorder-landing-geometry')) {
          if (++work > landingStageWorkLimit)
            return { status: 'unavailable', reason: 'work-budget' };
          for (const point of intersections(edge, boundary))
            divisions.push(requireValue(parameter(edge, point), 'reorder-landing-geometry'));
        }
      }
      const ordered = [...new Set(divisions)].sort((a, b) => a - b);
      for (let position = 1; position < ordered.length; position++) {
        const start = requireValue(ordered[position - 1], 'reorder-landing-geometry'),
          end = requireValue(ordered[position], 'reorder-landing-geometry');
        if (end - start <= epsilon) continue;
        const middle = at(edge, (start + end) / 2);
        let occluded = false;
        for (const other of overlaps) {
          if (++work > landingStageWorkLimit)
            return { status: 'unavailable', reason: 'work-budget' };
          const value = inset(requireValue(bubbles[other], 'reorder-landing-geometry'), middle);
          let sharedInterior = false;
          if (Math.abs(value) <= epsilon) {
            const direction = tangent(edge, (start + end) / 2);
            for (const boundary of requireValue(boundaries[other], 'reorder-landing-geometry')) {
              if (++work > landingStageWorkLimit)
                return { status: 'unavailable', reason: 'work-budget' };
              const progress = parameter(boundary, middle);
              if (progress === null) continue;
              const opposite = tangent(boundary, progress);
              if (direction.x * opposite.x + direction.y * opposite.y < -1 + epsilon) {
                sharedInterior = true;
                break;
              }
            }
          }
          if (
            value > epsilon ||
            (Math.abs(value) <= epsilon && (sharedInterior || other < index))
          ) {
            occluded = true;
            break;
          }
        }
        if (occluded) continue;
        const from = at(edge, start),
          to = at(edge, end);
        if (key(from) === key(to)) continue;
        pieces.push({
          from,
          to,
          command:
            edge.kind === 'line'
              ? `L${scalar(to.x)} ${scalar(to.y)}`
              : `A${edge.radius} ${edge.radius} 0 0 1 ${scalar(to.x)} ${scalar(to.y)}`,
        });
        if (pieces.length > 8192) return { status: 'unavailable', reason: 'work-budget' };
      }
    }
  if (!pieces.length) return { status: 'unavailable', reason: 'incomplete-contour' };
  const starts = new Map<string, number[]>();
  pieces.forEach((piece, index) => {
    const name = key(piece.from),
      list = starts.get(name) ?? [];
    list.push(index);
    starts.set(name, list);
  });
  const remaining = new Set(pieces.map((_piece, index) => index)),
    paths: string[] = [];
  while (remaining.size) {
    const first = requireValue(remaining.values().next().value, 'reorder-landing-geometry'),
      beginning = key(requireValue(pieces[first], 'reorder-landing-geometry').from);
    let position = first;
    const commands = [
      `M${scalar(requireValue(pieces[first], 'reorder-landing-geometry').from.x)} ${scalar(requireValue(pieces[first], 'reorder-landing-geometry').from.y)}`,
    ];
    while (remaining.has(position)) {
      const piece = requireValue(pieces[position], 'reorder-landing-geometry');
      remaining.delete(position);
      commands.push(piece.command);
      const ending = key(piece.to);
      if (ending === beginning) {
        commands.push('Z');
        break;
      }
      let next: number | undefined;
      for (const candidate of starts.get(ending) ?? []) {
        if (++work > landingStageWorkLimit) return { status: 'unavailable', reason: 'work-budget' };
        if (remaining.has(candidate)) {
          next = candidate;
          break;
        }
      }
      if (next === undefined) return { status: 'unavailable', reason: 'incomplete-contour' };
      position = next;
    }
    paths.push(commands.join(''));
  }
  const left = Math.min(...bubbles.map((bubble) => bubble.left)) - 1,
    top = Math.min(...bubbles.map((bubble) => bubble.top)) - 1;
  return Object.freeze({
    status: 'ready',
    path: paths.join(''),
    left,
    top,
    width: Math.max(...bubbles.map((bubble) => bubble.right)) + 1 - left,
    height: Math.max(...bubbles.map((bubble) => bubble.bottom)) + 1 - top,
  });
}
