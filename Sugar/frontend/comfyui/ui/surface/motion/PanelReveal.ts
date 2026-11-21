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
/** Slide a docked panel in from its edge and back out, with the shared surface motion. */
import { surfaceMotion } from '../appearance/SurfaceMotionTokens.js';

/** The window edge a docked panel lives against. */
export type PanelEdge = 'bottom' | 'right';

/** The panel's resting place and its place just past its edge, faded out. */
function frames(edge: PanelEdge): Keyframe[] {
  const away = edge === 'bottom' ? 'translateY(100%)' : 'translateX(100%)';
  return [
    { transform: away, opacity: 0 },
    { transform: 'none', opacity: 1 },
  ];
}

/** Whether motion runs: reduced motion and engines without animations make every change instant. */
function moves(element: HTMLElement): boolean {
  const reduced = element.ownerDocument.defaultView?.matchMedia?.(
    '(prefers-reduced-motion: reduce)',
  ).matches;
  return !reduced && typeof element.animate === 'function';
}

/** Own the enter and exit motion of docked panels; an exit can be taken back by a new enter. */
export class PanelReveal {
  readonly #leaving = new Map<HTMLElement, { animation: Animation; done: () => void }>();

  /** Slide `element` in from `edge`; an exit still running on it is taken back instead. */
  enter(element: HTMLElement, edge: PanelEdge): void {
    const leaving = this.#leaving.get(element);
    if (leaving) {
      this.#leaving.delete(element);
      leaving.animation.reverse();
      return;
    }
    if (!moves(element)) return;
    element.animate(frames(edge), {
      duration: surfaceMotion.duration,
      easing: surfaceMotion.easing,
    });
  }

  /** Slide `element` out to `edge`, then `done`; at once when motion is off. */
  exit(element: HTMLElement, edge: PanelEdge, done: () => void): void {
    if (!moves(element)) {
      done();
      return;
    }
    const animation = element.animate([...frames(edge)].reverse(), {
      duration: surfaceMotion.exitDuration,
      easing: surfaceMotion.easing,
      fill: 'forwards',
    });
    this.#leaving.set(element, { animation, done });
    animation.addEventListener('finish', () => {
      // A taken-back exit played back to rest: nothing leaves, the effect only ends.
      if (animation.playbackRate < 0) {
        animation.cancel();
        return;
      }
      if (this.#leaving.get(element)?.animation !== animation) return;
      this.#leaving.delete(element);
      done();
      animation.cancel();
    });
  }

  /** Whether `element` is sliding out. */
  leaving(element: HTMLElement): boolean {
    return this.#leaving.has(element);
  }

  /** End every exit at once, as its completion would. */
  dispose(): void {
    const leaving = [...this.#leaving.values()];
    this.#leaving.clear();
    for (const { animation, done } of leaving) {
      done();
      animation.cancel();
    }
  }
}
