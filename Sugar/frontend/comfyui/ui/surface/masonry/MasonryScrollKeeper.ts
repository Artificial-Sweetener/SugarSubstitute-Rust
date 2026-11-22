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
/** Bring a workflow's masonry back to where it was left, and keep where it is left again. */
import type { WorkflowScroll, WorkflowScrollMemory } from '../../workflow/domain/WorkflowView.js';
import { captureMasonryScroll, masonryScrollTop } from './MasonryCubeSections.js';

/** Input by which a person takes the masonry's scroll over. */
const TAKEOVER = ['wheel', 'pointerdown', 'keydown', 'touchstart'] as const;

/**
 * Keep one mounted masonry's scroll for its workflow.
 *
 * Once the cards have entered, the kept scroll is brought back, unless something scrolled the
 * masonry while they entered. Cards can still be growing then, so a kept scroll the masonry cannot
 * reach yet stays pending and is applied again on each layout, until it lands or the person
 * scrolls. Where the masonry rests is recorded only once that is settled, so a scroll the cards
 * could not reach yet is never written over the kept one, and neither is a masonry that is no
 * longer laid out.
 */
export class MasonryScrollKeeper {
  readonly #startTop: number;
  #pending: WorkflowScroll | null = null;
  #settled = false;
  /** Where the last application left the masonry, so its own scroll event is not a takeover. */
  #applied: number | null = null;
  /** The cards have entered, so a newer kept scroll applies at once. */
  #entered = false;
  readonly #unsubscribe: (() => void) | null;

  /** Watch one mounted masonry element for the workflow `memory` belongs to. */
  constructor(
    private readonly element: HTMLElement,
    private readonly memory: WorkflowScrollMemory | null,
    private readonly report: (error: unknown) => void,
  ) {
    this.#startTop = element.scrollTop;
    element.addEventListener('scrollend', this.#record);
    element.addEventListener('scroll', this.#moved, { passive: true });
    element.addEventListener('sc-masonry-layout', this.#retry);
    for (const name of TAKEOVER) element.addEventListener(name, this.#takeOver, { passive: true });
    this.#unsubscribe = memory?.subscribe(this.#restored) ?? null;
  }

  /** The cards have entered: bring back the kept scroll unless something scrolled meanwhile. */
  settle(): void {
    this.#entered = true;
    this.#pending =
      this.element.scrollTop === this.#startTop ? (this.memory?.read() ?? null) : null;
    this.#apply();
  }

  /** Record where the masonry rests, if settled, and stop watching it. */
  dispose(): void {
    this.#unsubscribe?.();
    this.element.removeEventListener('scrollend', this.#record);
    this.element.removeEventListener('scroll', this.#moved);
    this.element.removeEventListener('sc-masonry-layout', this.#retry);
    for (const name of TAKEOVER) this.element.removeEventListener(name, this.#takeOver);
    this.#record();
  }

  #apply(): void {
    const kept = this.#pending;
    if (kept) {
      const top = masonryScrollTop(this.element, kept);
      if (top !== null) this.element.scrollTop = top;
      if (top === null || Math.abs(this.element.scrollTop - top) > 1) {
        this.#applied = this.element.scrollTop;
        return;
      }
    }
    this.#pending = null;
    this.#settled = true;
    this.#record();
  }

  /** A restored session brought a newer kept scroll: the masonry goes there. */
  readonly #restored = (): void => {
    if (!this.#entered) return;
    this.#pending = this.memory?.read() ?? null;
    this.#settled = false;
    this.#apply();
  };

  readonly #retry = (): void => {
    if (this.#pending) this.#apply();
  };

  readonly #takeOver = (): void => {
    if (!this.#pending) return;
    this.#pending = null;
    this.#settled = true;
  };

  readonly #moved = (): void => {
    if (this.#pending && this.element.scrollTop !== this.#applied) this.#takeOver();
  };

  readonly #record = (): void => {
    // A masonry taken out of the document or hidden reads as scrolled to its top; only one that
    // is laid out tells where it rests.
    if (!this.#settled || !this.memory || !this.element.isConnected || !this.element.clientHeight)
      return;
    try {
      this.memory.record(captureMasonryScroll(this.element));
    } catch (error: unknown) {
      this.report(error);
    }
  };
}
