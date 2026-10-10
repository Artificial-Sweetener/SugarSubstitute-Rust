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
/** Animate the content's inset on the compositor after committing one settled layout destination. */
import { fluentMotion } from '../appearance/FluentThemeRuntime.js';

/** Retain and interrupt one horizontal rail-reservation effect without frame-by-frame packing. */
export class MasonryStackInsetMotion {
  #animation: Animation | null = null;
  #width: number | null = null;

  /** Commit rail space once and animate the retained content from its painted location. */
  set(element: HTMLElement, content: HTMLElement, width: number, layout: () => void): void {
    if (this.#width === width) return;
    const before = content.getBoundingClientRect().left;
    const initial = this.#width === null;
    this.#width = width;
    this.#animation?.cancel();
    this.#animation = null;
    element.style.setProperty('--sc-masonry-stack-width', `${String(width)}px`);
    layout();
    const delta = before - content.getBoundingClientRect().left;
    if (
      initial ||
      Math.abs(delta) < 0.5 ||
      typeof content.animate !== 'function' ||
      element.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    )
      return;
    const animation = content.animate(
      [{ transform: `translateX(${String(delta)}px)` }, { transform: 'translateX(0px)' }],
      { duration: fluentMotion.duration, easing: fluentMotion.easing },
    );
    this.#animation = animation;
    void Promise.allSettled([animation.finished]).then(() => {
      if (this.#animation === animation) {
        animation.cancel();
        this.#animation = null;
      }
    });
  }

  /** Await the latest reserved-width destination before measuring graph travel endpoints. */
  async settled(): Promise<void> {
    while (this.#animation) await Promise.allSettled([this.#animation.finished]);
  }

  /** Cancel motion before the content is retired. */
  dispose(): void {
    this.#animation?.cancel();
    this.#animation = null;
  }
}
