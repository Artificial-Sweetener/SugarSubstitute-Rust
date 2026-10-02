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
/** Fade the workflow surface independently of card geometry, layout, and theme. */
export class WorkflowMasonryTransition {
  #animation: Animation | null = null;
  #surface: HTMLElement | null = null;

  /** Reverse from displayed opacity and retain the destination after the effect ends. */
  async run(surface: HTMLElement, entering: boolean, reducedMotion: boolean): Promise<void> {
    const from =
      this.#animation && this.#surface === surface && !reducedMotion
        ? (surface.ownerDocument.defaultView?.getComputedStyle(surface).opacity ??
          surface.style.opacity)
        : surface.style.opacity || '1';
    this.cancel();
    const to = entering ? '1' : '0';
    surface.style.opacity = to;
    if (reducedMotion || from === to || typeof surface.animate !== 'function') return;
    const animation = surface.animate([{ opacity: from }, { opacity: to }], {
      duration: 180,
      easing: 'ease-in-out',
      fill: 'both',
    });
    this.#animation = animation;
    this.#surface = surface;
    await Promise.allSettled([animation.finished]);
    if (this.#animation !== animation) return;
    this.cancel();
  }

  /** Release the active effect before reversal, renderer teardown, or graph replacement. */
  cancel(): void {
    this.#animation?.cancel();
    this.#animation = null;
    this.#surface = null;
  }
}
