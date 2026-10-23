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
/** Adapt scroll owners and their clipping ancestors into viewport-space handle tracks. */
export interface ScrollbarTrack {
  left: number;
  top: number;
  width: number;
  height: number;
  horizontalBottom: number;
  rtl: boolean;
}

/** Traverse open shadow boundaries as part of the same visual scroll hierarchy. */
export function scrollParent(element: HTMLElement): HTMLElement | null {
  const root = element.getRootNode();
  return (
    element.parentElement ??
    (root instanceof ShadowRoot && root.host instanceof HTMLElement ? root.host : null)
  );
}

/** Read an outward horizontal paint allowance in screen pixels, bounded by the drag-target size. */
export function scrollbarHorizontalOutset(element: HTMLElement): number {
  const value = Number(element.getAttribute('data-sc-scrollbar-horizontal-outset'));
  return Number.isFinite(value) ? Math.min(12, Math.max(0, value)) : 0;
}

/** Preserve a classic scrollbar's full owner extent or clip a floating handle to the viewport. */
export function scrollbarTrack(
  element: HTMLElement,
  preserveOwnerExtent = false,
): ScrollbarTrack | null {
  const windowRef = element.ownerDocument.defaultView;
  if (!windowRef || !element.isConnected) return null;
  const root = element === element.ownerDocument.scrollingElement;
  const rect = element.getBoundingClientRect();
  const scaleX = element.offsetWidth ? rect.width / element.offsetWidth : 1;
  const scaleY = element.offsetHeight ? rect.height / element.offsetHeight : 1;
  let left = root ? 0 : rect.left + element.clientLeft * scaleX;
  let top = root ? 0 : rect.top + element.clientTop * scaleY;
  let right = root
    ? windowRef.innerWidth
    : Math.min(rect.right, left + element.clientWidth * scaleX);
  let bottom = root
    ? windowRef.innerHeight
    : Math.min(rect.bottom, top + element.clientHeight * scaleY);
  const bottomInset = Number.parseFloat(windowRef.getComputedStyle(element).scrollPaddingBottom);
  if (Number.isFinite(bottomInset))
    bottom = Math.max(top, bottom - Math.max(0, bottomInset) * scaleY);
  let horizontalBottom = bottom + scrollbarHorizontalOutset(element);
  const edge =
    element.getAttribute('data-sc-scrollbar-edge') === 'parent' ? scrollParent(element) : null;
  if (edge) right = edge.getBoundingClientRect().right - 1;
  for (
    let parent = preserveOwnerExtent ? null : scrollParent(element);
    parent && !root;
    parent = scrollParent(parent)
  ) {
    const style = windowRef.getComputedStyle(parent);
    const bounds = parent.getBoundingClientRect();
    if (/(auto|scroll|hidden|clip)/.test(style.overflowX)) {
      left = Math.max(left, bounds.left);
      right = Math.min(right, bounds.right);
    }
    if (/(auto|scroll|hidden|clip)/.test(style.overflowY)) {
      top = Math.max(top, bounds.top);
      bottom = Math.min(bottom, bounds.bottom);
      horizontalBottom = Math.min(horizontalBottom, bounds.bottom);
    }
  }
  if (!preserveOwnerExtent) {
    left = Math.max(0, left);
    top = Math.max(0, top);
    right = Math.min(windowRef.innerWidth, right);
    bottom = Math.min(windowRef.innerHeight, bottom);
    horizontalBottom = Math.min(windowRef.innerHeight, horizontalBottom);
  }
  if (right - left <= 12 || bottom - top <= 12) return null;
  return {
    left,
    top,
    width: right - left,
    height: bottom - top,
    horizontalBottom,
    rtl: windowRef.getComputedStyle(element).direction === 'rtl',
  };
}
