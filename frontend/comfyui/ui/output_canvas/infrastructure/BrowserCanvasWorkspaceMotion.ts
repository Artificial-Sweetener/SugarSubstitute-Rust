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
/** Slide one workspace pane while retaining outgoing pixels until the compositor settles. */
import { fluentMotion } from '../../surface/appearance/FluentThemeRuntime.js';

/** Own reversible pane motion independently of workflow preferences and graph allocation. */
export class BrowserCanvasWorkspaceMotion {
  #effect: Animation | null = null;
  #generation = 0;
  #visible = false;
  #disposed = false;

  /** Retain the pane and publish only settled attachment availability changes. */
  constructor(
    private readonly element: HTMLElement,
    private readonly settled: () => void,
  ) {
    this.restore(false);
  }

  /** Keep the outgoing attachment alive only while its finite exit effect owns visible pixels. */
  get present(): boolean {
    return !this.#disposed && (this.#visible || this.#effect !== null);
  }

  /** Restore lifecycle and workflow endpoints without replaying user-toggle animation. */
  restore(visible: boolean): void {
    if (this.#disposed) return;
    this.#generation++;
    this.#visible = visible;
    this.#complete();
  }

  /** Reverse from the currently painted transform and respect reduced-motion preferences. */
  async setVisible(visible: boolean): Promise<void> {
    if (this.#disposed || this.#visible === visible) return;
    const viewport = this.element.ownerDocument.defaultView;
    const from = this.#effect
      ? (viewport?.getComputedStyle(this.element).transform ?? 'none')
      : this.#visible
        ? 'translateX(0)'
        : 'translateX(100%)';
    const generation = ++this.#generation;
    this.#visible = visible;
    this.#cancel();
    if (
      typeof this.element.animate !== 'function' ||
      viewport?.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    ) {
      this.#complete();
      return;
    }
    this.element.hidden = false;
    this.element.inert = !visible;
    this.element.dataset.canvasMotion = visible ? 'enter' : 'exit';
    try {
      this.#effect = this.element.animate(
        [{ transform: from }, { transform: visible ? 'translateX(0)' : 'translateX(100%)' }],
        { duration: fluentMotion.duration, easing: fluentMotion.easing, fill: 'both' },
      );
      await this.#effect.finished;
    } catch (error: unknown) {
      if (generation !== this.#generation || this.#disposed) return;
      this.#complete();
      this.settled();
      throw new Error('Output canvas slide failed.', { cause: error });
    }
    if (generation !== this.#generation || this.#disposed) return;
    this.#complete();
    this.settled();
  }

  /** Cancel pending completion before the pane and its image attachment are removed. */
  dispose(): void {
    if (this.#disposed) return;
    this.restore(false);
    this.#disposed = true;
  }

  /** Commit visibility before clearing effects so exiting pixels never flash back onscreen. */
  #complete(): void {
    this.element.hidden = !this.#visible;
    this.element.inert = !this.#visible;
    this.#cancel();
    delete this.element.dataset.canvasMotion;
  }

  /** Retire only the effect owned by this pane; superseded promises observe their generation. */
  #cancel(): void {
    const effect = this.#effect;
    this.#effect = null;
    effect?.cancel();
  }
}
