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
/** Read the masonry's Cube sections from their settled layout, and keep a scroll by its Cube. */
import type { WorkflowScroll } from '../../workflow/domain/WorkflowView.js';

/** One Cube's section in the masonry's scroll coordinates. */
export interface MasonryCubeSection {
  readonly id: string;
  readonly y: number;
  readonly height: number;
}

const CUBE_SECTION = 'cube:';

/** The Cube sections the masonry shows, top to bottom, at their settled layout positions. */
export function masonryCubeSections(element: HTMLElement): MasonryCubeSection[] {
  return [...element.querySelectorAll<HTMLElement>('header[data-masonry-section]')]
    .flatMap((header) => {
      const section = header.dataset.masonrySection ?? '';
      if (!section.startsWith(CUBE_SECTION) || header.hasAttribute('data-sc-search-card-hidden'))
        return [];
      const y = Number(header.dataset.masonryY) + (header.parentElement?.offsetTop ?? 0);
      const height = Number(header.dataset.masonryHeight);
      return Number.isFinite(y) && Number.isFinite(height) && height >= 0
        ? [{ id: section.slice(CUBE_SECTION.length), y, height }]
        : [];
    })
    .sort((a, b) => a.y - b.y);
}

/** Where the masonry is scrolled: the Cube whose section holds its top, and how far into it. */
export function captureMasonryScroll(element: HTMLElement): WorkflowScroll {
  const top = Math.max(0, element.scrollTop);
  const holding = masonryCubeSections(element)
    .filter((section) => section.y <= top)
    .at(-1);
  return holding
    ? { cube: holding.id, offset: Math.round(top - holding.y) }
    : { cube: null, offset: Math.round(top) };
}

/**
 * Scroll the masonry back to a kept scroll: the same distance into the same Cube, or the top when
 * that Cube is gone.
 */
export function restoreMasonryScroll(element: HTMLElement, scroll: WorkflowScroll | null): void {
  if (!scroll) {
    element.scrollTop = 0;
    return;
  }
  if (scroll.cube === null) {
    element.scrollTop = scroll.offset;
    return;
  }
  const section = masonryCubeSections(element).find((item) => item.id === scroll.cube);
  element.scrollTop = section ? section.y + scroll.offset : 0;
}
