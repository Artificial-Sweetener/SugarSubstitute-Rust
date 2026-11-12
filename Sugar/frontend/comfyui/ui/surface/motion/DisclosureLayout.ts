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
/** Share settled disclosure geometry with surrounding presentation layout. */

export const DISCLOSURE_LAYOUT_EVENT = 'sugarcubes-disclosure-layout';

/** Publish one destination so masonry packs once instead of chasing animated heights. */
export function publishDisclosureHeight(target: HTMLElement, height: number | null): void {
  if (height === null) {
    if (!target.hasAttribute('data-sc-disclosure-height')) return;
    delete target.dataset.scDisclosureHeight;
  } else target.dataset.scDisclosureHeight = String(height);
  target.dispatchEvent(new Event(DISCLOSURE_LAYOUT_EVENT, { bubbles: true }));
}

/** Reserve the final card height while native content animates inside its clipped surface. */
export function measureDisclosureCardHeight(card: HTMLElement): number {
  let height = outerHeight(card);
  for (const target of card.querySelectorAll<HTMLElement>('[data-sc-disclosure-height]')) {
    if (target.parentElement?.closest('[data-sc-disclosure-height]')) continue;
    const destination = Number(target.dataset.scDisclosureHeight);
    const current = Number.parseFloat(getComputedStyle(target).height);
    if (Number.isFinite(destination) && Number.isFinite(current)) height += destination - current;
  }
  return Math.max(0, height);
}

/** Keep subpixel geometry in CSS units independently of camera transforms and integer offsets. */
function outerHeight(element: HTMLElement): number {
  const style = getComputedStyle(element);
  const height = Number.parseFloat(style.height);
  if (!Number.isFinite(height)) return element.offsetHeight;
  if (style.boxSizing === 'border-box') return height;
  return (
    height +
    ['padding-top', 'padding-bottom', 'border-top-width', 'border-bottom-width'].reduce(
      (sum, name) => sum + (Number.parseFloat(style.getPropertyValue(name)) || 0),
      0,
    )
  );
}
