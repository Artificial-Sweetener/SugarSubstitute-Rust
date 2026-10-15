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
/** Own transient tab transforms and settle motion without mutating workflow order. */
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
import type { WorkflowTabDragPlacement } from './WorkflowTabDragGeometry.js';

const motion = { duration: 180, easing: 'cubic-bezier(.22,1,.36,1)' };

/** Retain visual continuity across the host's accepted DOM permutation. */
export class WorkflowTabDragPreview {
  readonly #animations = new Map<Animation, () => void>();
  readonly #elementAnimations = new Map<HTMLElement, Animation>();
  #active = false;

  /** Restore canonical orb destinations when accepted order is projected. */
  constructor(
    private readonly list: HTMLElement,
    private readonly viewport: HTMLElement,
    private readonly restored: () => void,
  ) {}

  /** Report whether the pointer preview owns geometry before accepted order is projected. */
  get active(): boolean {
    return this.#active;
  }

  /** Move the held tab immediately and animate displaced neighbors from stable slots. */
  show(tabs: readonly HTMLElement[], origin: number, placement: WorkflowTabDragPlacement): void {
    if (!this.#active) {
      this.#captureBackdrop();
      this.#cancelElement(this.list);
    }
    this.#active = true;
    this.viewport.setAttribute('data-sc-tab-dragging', '');
    const first = Math.floor(this.viewport.scrollLeft / WORKFLOW_TAB_WIDTH);
    const progress = Math.max(
      0,
      Math.min(1, 1 - (placement.draggedLeft / WORKFLOW_TAB_WIDTH - first)),
    );
    const successor = origin === first ? tabs.findIndex((_, index) => index === first + 1) : first;
    for (const [index, tab] of tabs.entries()) {
      const offset = placement.offsets[index] ?? 0;
      const destination = `translateX(${String(offset)}px)`;
      if (index === origin) {
        this.#cancelElement(tab);
        tab.style.transition = 'none';
        tab.style.transform = destination;
      } else if (tab.style.transform !== destination) {
        const before = tab.getBoundingClientRect().left;
        this.#cancelElement(tab);
        tab.style.transition = 'none';
        tab.style.transform = destination;
        const from = offset + before - tab.getBoundingClientRect().left;
        if (from !== offset && !this.#reducedMotion() && typeof tab.animate === 'function')
          this.#animate(
            tab,
            [{ transform: `translateX(${String(from)}px)` }, { transform: destination }],
            undefined,
            150,
          );
      }
      tab.toggleAttribute('data-sc-held', index === origin);
      const cutout = index === origin ? progress : index === successor ? 1 - progress : 0;
      tab.style.paddingLeft = `${String(20 + 12 * cutout)}px`;
      tab.style.setProperty('--sc-tab-cutout-radius', `${String(25 * cutout)}px`);
      tab.style.setProperty('--sc-tab-cutout-x', `${String(-17 * cutout)}px`);
      if (tab.getAttribute('aria-selected') === 'true')
        this.list.style.setProperty(
          '--sc-selected-offset',
          `${String(index * WORKFLOW_TAB_WIDTH + (placement.offsets[index] ?? 0))}px`,
        );
    }
    this.list.style.setProperty('--sc-tab-scroll', `${String(this.viewport.scrollLeft)}px`);
  }

  /** Composite the translucent held painter against its frame, obscuring covered sibling text. */
  #captureBackdrop(): void {
    const window = this.list.ownerDocument.defaultView;
    if (!window) return;
    for (let parent = this.list.parentElement; parent; parent = parent.parentElement) {
      const color = window.getComputedStyle(parent).backgroundColor;
      if (color && color !== 'transparent' && color !== 'rgba(0, 0, 0, 0)') {
        this.list.style.setProperty('--sc-tab-drag-backdrop', color);
        return;
      }
    }
    this.list.style.removeProperty('--sc-tab-drag-backdrop');
  }

  /** Capture painted positions once before a canonical DOM reorder or cancellation. */
  capture(tabs: readonly HTMLElement[]): ReadonlyMap<HTMLElement, number> {
    return new Map(tabs.map((tab) => [tab, tab.getBoundingClientRect().left]));
  }

  /** Animate from the currently painted position into each accepted slot without a jump. */
  settle(tabs: readonly HTMLElement[], positions: ReadonlyMap<HTMLElement, number>): void {
    this.#cancelAnimations();
    this.viewport.removeAttribute('data-sc-tab-dragging');
    const selectedOffset = this.list.style.getPropertyValue('--sc-selected-offset');
    const starts = tabs.map((tab) => ({
      tab,
      radius: tab.style.getPropertyValue('--sc-tab-cutout-radius'),
      x: tab.style.getPropertyValue('--sc-tab-cutout-x'),
      padding: tab.style.paddingLeft,
    }));
    for (const tab of tabs) {
      tab.style.transition = 'none';
      tab.style.removeProperty('transform');
      tab.style.removeProperty('--sc-tab-cutout-radius');
      tab.style.removeProperty('--sc-tab-cutout-x');
      tab.style.removeProperty('padding-left');
    }
    this.#active = false;
    this.restored();
    for (const { tab, radius, x, padding } of starts) {
      const before = positions.get(tab);
      const delta = before === undefined ? 0 : before - tab.getBoundingClientRect().left;
      const destinationRadius = tab.hasAttribute('data-orb-adjacent') ? '25px' : '0px';
      const destinationX = tab.style.getPropertyValue('--sc-tab-cutout-x') || '-17px';
      const destinationPadding = tab.hasAttribute('data-orb-adjacent') ? '32px' : '20px';
      const finished = (): void => {
        tab.removeAttribute('data-sc-held');
        tab.style.removeProperty('transition');
      };
      if (
        (delta || radius !== destinationRadius) &&
        !this.#reducedMotion() &&
        typeof tab.animate === 'function'
      )
        this.#animate(
          tab,
          [
            {
              transform: `translateX(${String(delta)}px)`,
              '--sc-tab-cutout-radius': radius || '0px',
              '--sc-tab-cutout-x': x || '-17px',
              paddingLeft: padding || destinationPadding,
            },
            {
              transform: 'translateX(0px)',
              '--sc-tab-cutout-radius': destinationRadius,
              '--sc-tab-cutout-x': destinationX,
              paddingLeft: destinationPadding,
            },
          ],
          finished,
        );
      else finished();
    }
    const destination = this.list.style.getPropertyValue('--sc-selected-offset');
    if (
      selectedOffset !== destination &&
      !this.#reducedMotion() &&
      typeof this.list.animate === 'function'
    )
      this.#animate(this.list, [
        { '--sc-selected-offset': selectedOffset },
        { '--sc-selected-offset': destination },
      ]);
  }

  /** Track native animations without retaining rejected completion promises. */
  #animate(
    element: HTMLElement,
    frames: Keyframe[],
    finished?: () => void,
    duration = motion.duration,
  ): void {
    const animation = element.animate(frames, { ...motion, duration });
    const cleanup = (): void => {
      this.#animations.delete(animation);
      if (this.#elementAnimations.get(element) === animation)
        this.#elementAnimations.delete(element);
      finished?.();
    };
    this.#animations.set(animation, cleanup);
    this.#elementAnimations.set(element, animation);
    animation.onfinish = animation.oncancel = cleanup;
  }

  /** Retarget only the affected visual element from its painted position. */
  #cancelElement(element: HTMLElement): void {
    const animation = this.#elementAnimations.get(element);
    if (!animation) return;
    const cleanup = this.#animations.get(animation);
    animation.onfinish = animation.oncancel = null;
    animation.cancel();
    cleanup?.();
  }

  /** Respect the host browser's accessibility preference at the animation boundary. */
  #reducedMotion(): boolean {
    return (
      this.list.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
        .matches ?? false
    );
  }

  /** Retire outstanding motion before a new preview or unmount. */
  #cancelAnimations(): void {
    for (const [animation, cleanup] of this.#animations) {
      animation.onfinish = animation.oncancel = null;
      animation.cancel();
      cleanup();
    }
    this.#animations.clear();
  }

  /** Release native animation resources when the frame is disconnected. */
  dispose(): void {
    this.#cancelAnimations();
  }
}
