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
/** Own interruptible presentation motion without changing graph geometry or native controls. */
import { fluentMotion } from '../appearance/FluentThemeRuntime.js';

export interface SurfaceRectangle {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Bind one retained native mount to its settled layout destination. */
export interface SurfacePlacement {
  element: HTMLElement;
  rectangle: SurfaceRectangle;
}

/** Animate stable surface items with one lifecycle for entry, placement, resize, and retirement. */
export class SurfaceMotion {
  readonly #placements = new Map<HTMLElement, SurfaceRectangle>();
  readonly #heights = new Map<HTMLElement, number>();
  readonly #moving = new Set<HTMLElement>();
  readonly #animations = new Map<HTMLElement, Animation>();
  #ready = false;

  /** Record initial placement before allowing subsequent membership and geometry motion. */
  settle(): void {
    this.#ready = true;
  }

  /** Commit owned placement destinations before another presentation animates their contents. */
  finishAnimations(): void {
    for (const element of this.#animations.keys()) this.cancel(element);
  }

  /** Read interrupted positions together before committing compositor placement destinations. */
  place(placements: readonly SurfacePlacement[], animate = true): void {
    const prepared = placements.flatMap(({ element, rectangle }) => {
      const previous = this.#placements.get(element);
      if (previous && sameRectangle(previous, rectangle)) return [];
      const interrupted = this.#animations.has(element)
        ? element.ownerDocument.defaultView?.getComputedStyle(element).transform
        : undefined;
      return [{ element, rectangle, previous, interrupted }];
    });
    for (const { element, rectangle, previous, interrupted } of prepared)
      this.#place(element, rectangle, previous, interrupted, animate);
  }

  /** Apply a prepared destination without another layout read between neighbor writes. */
  #place(
    element: HTMLElement,
    rectangle: SurfaceRectangle,
    previous: SurfaceRectangle | undefined,
    interrupted: string | undefined,
    animate: boolean,
  ): void {
    const target = `translate(${String(rectangle.x)}px, ${String(rectangle.y)}px)`;
    this.cancel(element);
    this.#placements.set(element, rectangle);
    element.style.transform = target;
    element.style.transformOrigin = '0 0';
    if (
      !this.#ready ||
      !animate ||
      (previous && previous.x === rectangle.x && previous.y === rectangle.y)
    )
      return;
    const from =
      interrupted && interrupted !== 'none'
        ? interrupted
        : previous
          ? `translate(${String(previous.x)}px, ${String(previous.y)}px)`
          : target;
    this.#run(
      element,
      [
        { transform: from, opacity: previous ? 1 : 0 },
        { transform: target, opacity: 1 },
      ],
      fluentMotion.duration,
    );
    if (this.#animations.has(element)) {
      this.#moving.add(element);
      element.dataset.scSurfaceMoving = '';
    }
  }

  /** Resize the scroll extent continuously so closing cards cannot clamp scrolling in one jump. */
  resizeHeight(element: HTMLElement, height: number): void {
    const previous = this.#heights.get(element);
    if (previous !== undefined && Math.abs(previous - height) < 0.5) return;
    const from =
      this.#ready && previous !== undefined
        ? Number.parseFloat(
            element.ownerDocument.defaultView?.getComputedStyle(element).height || '0',
          )
        : height;
    this.cancel(element);
    this.#heights.set(element, height);
    element.style.height = `${String(height)}px`;
    if (!this.#ready || previous === undefined || Math.abs(from - height) < 0.5) return;
    this.#run(
      element,
      [{ height: `${String(from)}px` }, { height: `${String(height)}px` }],
      fluentMotion.duration,
    );
  }

  /** Keep retiring controls mounted until their exit finishes, allowing immediate resurrection. */
  retire(element: HTMLElement, release: () => void): void {
    const opacity = element.ownerDocument.defaultView?.getComputedStyle(element).opacity || '1';
    this.cancel(element);
    this.#run(element, [{ opacity }, { opacity: 0 }], fluentMotion.exitDuration, () => {
      this.forget(element);
      release();
    });
  }

  /** Reverse retirement without replacing an item that becomes visible again. */
  revive(element: HTMLElement): void {
    const opacity = element.ownerDocument.defaultView?.getComputedStyle(element).opacity || '1';
    this.cancel(element);
    this.#run(element, [{ opacity }, { opacity: 1 }], fluentMotion.duration);
  }

  /** Cancel one effect while preserving the committed destination geometry. */
  cancel(element: HTMLElement): void {
    const animation = this.#animations.get(element);
    this.#animations.delete(element);
    animation?.cancel();
    if (this.#moving.delete(element)) delete element.dataset.scSurfaceMoving;
  }

  /** Release one item before its native mount or section is disposed. */
  forget(element: HTMLElement): void {
    this.cancel(element);
    this.#placements.delete(element);
    this.#heights.delete(element);
  }

  /** Stop every effect before the scroll surface leaves graph presentation. */
  dispose(): void {
    for (const element of this.#animations.keys()) this.cancel(element);
    this.#placements.clear();
    this.#heights.clear();
  }

  /** Respect live reduced-motion preferences and suppress stale completion callbacks. */
  #run(element: HTMLElement, frames: Keyframe[], duration: number, complete?: () => void): void {
    const reduced = element.ownerDocument.defaultView?.matchMedia?.(
      '(prefers-reduced-motion: reduce)',
    ).matches;
    if (reduced || typeof element.animate !== 'function') {
      complete?.();
      return;
    }
    const animation = element.animate(frames, {
      duration,
      easing: fluentMotion.easing,
      fill: 'both',
    });
    this.#animations.set(element, animation);
    void Promise.allSettled([animation.finished]).then(() => {
      if (this.#animations.get(element) !== animation) return;
      this.cancel(element);
      complete?.();
    });
  }
}

/** Avoid restarting effects for duplicate ResizeObserver deliveries. */
function sameRectangle(left: SurfaceRectangle, right: SurfaceRectangle): boolean {
  const keys: readonly (keyof SurfaceRectangle)[] = ['x', 'y', 'width', 'height'];
  return keys.every((key) => {
    return Math.abs(left[key] - right[key]) < 0.5;
  });
}
