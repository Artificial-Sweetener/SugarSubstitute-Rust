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
/**
 * Let a node card hide the cards beneath it in the desktop frame. A card there is a translucent
 * fill over the window material, so a card that lies under another would show through it, which
 * an opaque Comfy card never does. Each card that another card covers gets a mask: the parts the
 * cards above it cover are taken out, as strongly as ComfyUI's node opacity setting says, so at 1
 * nothing of it shows through and lower settings let it through in proportion, as in Comfy.
 * Cards stack as Comfy stacks them: by their z-index, then by their order in the graph.
 */

/** One node card: its element, its bounds in graph units and its place in the stack. */
export interface StackedCard {
  readonly element: HTMLElement;
  readonly bounds: readonly [number, number, number, number];
  readonly order: number;
}

/** A covered part of a card, in the card's own units. */
export interface CoveredRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** How far a card's ports and handles stand out of its box; the mask leaves them alone. */
const OUTSET = 32;
/** A card's corner radius in graph units, as its own CSS rounds it. */
const RADIUS = 4;
/** The mask each element was last given, so an unchanged stack writes nothing. */
const applied = new WeakMap<HTMLElement, string>();

/** For each card, the parts of it the cards above it cover. */
export function coveredParts(cards: readonly StackedCard[]): Map<StackedCard, CoveredRect[]> {
  const stacked = [...cards].sort((a, b) => a.order - b.order);
  const result = new Map<StackedCard, CoveredRect[]>();
  stacked.forEach((card, index) => {
    const [x, y, width, height] = card.bounds;
    const parts: CoveredRect[] = [];
    for (const above of stacked.slice(index + 1)) {
      const [ax, ay, aw, ah] = above.bounds;
      const left = Math.max(x, ax);
      const top = Math.max(y, ay);
      const right = Math.min(x + width, ax + aw);
      const bottom = Math.min(y + height, ay + ah);
      // The whole card above is taken out, so its rounded corners cut the card beneath as drawn.
      if (right > left && bottom > top) parts.push({ x: ax - x, y: ay - y, width: aw, height: ah });
    }
    result.set(card, parts);
  });
  return result;
}

/** The mask image for a card `width` × `height` with `parts` covered at `strength`. */
export function stackingMask(
  width: number,
  height: number,
  parts: readonly CoveredRect[],
  strength: number,
): string {
  const round = (value: number) => String(Math.round(value * 100) / 100);
  const holes = parts
    .map(
      (part) =>
        `<rect x='${round(part.x + OUTSET)}' y='${round(part.y + OUTSET)}' width='${round(part.width)}' height='${round(part.height)}' rx='${String(RADIUS)}' fill='black' fill-opacity='${round(strength)}'/>`,
    )
    .join('');
  const w = round(width + OUTSET * 2);
  const h = round(height + OUTSET * 2);
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'><rect width='${w}' height='${h}' fill='white'/>${holes}</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

/** Mask every card that cards above it cover; clear the mask of every other card. */
export function applyStacking(cards: readonly StackedCard[], strength: number): void {
  for (const [card, parts] of coveredParts(cards)) {
    const mask =
      strength > 0 && parts.length
        ? stackingMask(card.bounds[2], card.bounds[3], parts, strength)
        : '';
    if ((applied.get(card.element) ?? '') === mask) continue;
    applied.set(card.element, mask);
    const style = card.element.style;
    if (!mask) {
      for (const name of [
        'mask-image',
        'mask-mode',
        'mask-size',
        'mask-position',
        'mask-repeat',
        'mask-clip',
      ])
        style.removeProperty(name);
      continue;
    }
    style.setProperty('mask-image', mask);
    style.setProperty('mask-mode', 'luminance');
    style.setProperty('mask-repeat', 'no-repeat');
    style.setProperty('mask-clip', 'no-clip');
    style.setProperty('mask-position', `-${String(OUTSET)}px -${String(OUTSET)}px`);
    style.setProperty(
      'mask-size',
      `calc(100% + ${String(OUTSET * 2)}px) calc(100% + ${String(OUTSET * 2)}px)`,
    );
  }
}

/** Remove every mask this owner gave. */
export function clearStacking(cards: Iterable<HTMLElement>): void {
  for (const element of cards) {
    if (!applied.has(element)) continue;
    applied.delete(element);
    for (const name of [
      'mask-image',
      'mask-mode',
      'mask-size',
      'mask-position',
      'mask-repeat',
      'mask-clip',
    ])
      element.style.removeProperty(name);
  }
}
