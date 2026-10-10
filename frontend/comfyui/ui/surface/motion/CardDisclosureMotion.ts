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
/** Animate native card disclosure with clipped height, retaining content until closing completes. */
import { fluentMotion } from '../appearance/FluentThemeRuntime.js';
import { publishDisclosureHeight } from './DisclosureLayout.js';

export interface DisclosureGeometry {
  /** Reuse geometry read before a batch of native cards begins writing layout. */
  fromHeight?: number;
  /** Let content follow the animated extent only for a disclosure that moves its footer. */
  contentLayout?: 'fill';
  closedHeight?(): number;
  expandedHeight?(): number;
  settle?(): Promise<void>;
}

/** Own interruptible disclosure presentation without replacing native controls or persisted flags. */
export class CardDisclosureMotion {
  readonly #target: HTMLElement;
  #animation: Animation | null = null;
  #desired: boolean | null = null;
  #overflow: string | null = null;
  #height: { value: string; priority: string } | null = null;
  #apply: ((collapsed: boolean) => void) | null = null;
  #owner: object | null = null;
  #revision = 0;

  /** Bind the projected mount whose intrinsic height drives its existing masonry layout. */
  constructor(target: HTMLElement) {
    this.#target = target;
  }

  /** Read the visible CSS extent before the scoped native refresh batch changes any card. */
  captureHeight(): number {
    return height(this.#target);
  }

  /** Reverse from the current visible height and commit collapse only after content closes. */
  toggle(
    collapsed: boolean,
    apply: (collapsed: boolean) => void,
    geometry?: DisclosureGeometry,
    owner: object = this.#target,
  ): void {
    this.#revision++;
    const sameOwner = this.#owner === owner;
    const desired = !(sameOwner ? (this.#desired ?? collapsed) : collapsed);
    const from = geometry?.fromHeight ?? height(this.#target);
    const previousRoot =
      this.#target.querySelector<HTMLElement>('.lg-node') ??
      this.#target.closest<HTMLElement>('.lg-node');
    const canAnimate = previousRoot !== null && this.#canAnimate(previousRoot);
    if (!sameOwner && this.#desired !== null) this.#apply?.(this.#desired);
    this.#animation?.cancel();
    this.#animation = null;
    this.#desired = desired;
    this.#apply = apply;
    this.#owner = owner;
    const root =
      this.#target.querySelector<HTMLElement>('.lg-node') ??
      this.#target.closest<HTMLElement>('.lg-node');
    if (!root || !canAnimate) {
      apply(desired);
      this.#release();
      return;
    }
    this.#target.dataset.scDisclosureLayout = geometry?.contentLayout ?? 'intrinsic';
    if (geometry?.settle && !desired) {
      this.#hold(from);
      this.#target.dataset.scDisclosureTarget = 'expanded';
    }
    if (!desired) apply(false);
    if (geometry?.settle && !desired) {
      void geometry.settle().then(() => {
        if (this.#apply === apply && this.#desired === desired)
          this.#animate(root, desired, from, apply, geometry);
      });
    } else this.#animate(root, desired, from, apply, geometry);
  }

  /** Animate native widget membership changes without inventing a collapsed node state. */
  resize(apply: () => void, geometry?: DisclosureGeometry): void {
    if (this.#desired !== null) {
      apply();
      return;
    }
    const revision = ++this.#revision;
    const from = geometry?.fromHeight ?? height(this.#target);
    this.#animation?.cancel();
    this.#animation = null;
    const root = this.#target.querySelector<HTMLElement>('.lg-node');
    if (!root || !this.#canAnimate(root, true)) {
      try {
        apply();
      } finally {
        this.#release();
      }
      return;
    }
    this.#target.dataset.scDisclosureLayout = geometry?.contentLayout ?? 'intrinsic';
    this.#hold(from);
    try {
      apply();
    } catch (error: unknown) {
      this.#release();
      throw error;
    }
    const animate = (): void => {
      if (this.#revision !== revision) return;
      const to =
        geometry?.expandedHeight?.() ??
        height(this.#target.querySelector<HTMLElement>('.lg-node') ?? root);
      if (Math.abs(to - from) < 0.5) {
        this.#release();
        return;
      }
      this.#target.dataset.scDisclosureTarget = to < from ? 'collapsed' : 'expanded';
      this.#run(from, to, () => this.#release());
    };
    if (geometry?.settle) void geometry.settle().then(animate);
    else animate();
  }

  /** Apply the same appearance and accessibility motion policy to every disclosure trigger. */
  #canAnimate(root: HTMLElement, contentChange = false): boolean {
    return (
      (contentChange || root.hasAttribute('data-sc-fluent-node')) &&
      !this.#target.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
        .matches &&
      typeof this.#target.animate === 'function'
    );
  }

  /** Retain the current extent while native mounting and autosizing establish the destination. */
  #hold(from: number): void {
    if (this.#height === null)
      this.#height = {
        value: this.#target.style.getPropertyValue('height'),
        priority: this.#target.style.getPropertyPriority('height'),
      };
    this.#target.style.height = `${String(from)}px`;
    if (this.#overflow === null) this.#overflow = this.#target.style.overflow;
    this.#target.style.overflow = 'clip';
    this.#target.dataset.scDisclosing = '';
  }

  /** Animate either whole-card or field-container disclosure through the same motion policy. */
  #animate(
    root: HTMLElement,
    desired: boolean,
    from: number,
    apply: (collapsed: boolean) => void,
    geometry?: DisclosureGeometry,
  ): void {
    const header = root.querySelector<HTMLElement>('[data-testid^="node-header-"]');
    const wrapper = root.querySelector<HTMLElement>('[data-testid="node-inner-wrapper"]');
    const style = wrapper
      ? this.#target.ownerDocument.defaultView?.getComputedStyle(wrapper)
      : null;
    const to = desired
      ? (geometry?.closedHeight?.() ??
        (header
          ? height(header) +
            Number.parseFloat(style?.borderTopWidth || '0') +
            Number.parseFloat(style?.borderBottomWidth || '0')
          : height(root)))
      : (geometry?.expandedHeight?.() ?? height(root));
    if (this.#overflow === null) this.#overflow = this.#target.style.overflow;
    this.#target.style.overflow = 'clip';
    this.#target.dataset.scDisclosing = '';
    this.#target.dataset.scDisclosureTarget = desired ? 'collapsed' : 'expanded';
    this.#run(from, to, () => {
      if (desired) apply(true);
      const finish = (): void => this.#release();
      if (desired && geometry?.settle) {
        const revision = this.#revision;
        void geometry.settle().then(() => {
          if (this.#revision === revision) finish();
        });
      } else finish();
    });
  }

  /** Publish and animate one final extent through the shared motion and masonry policy. */
  #run(from: number, to: number, finish: () => void): void {
    const animation = this.#target.animate(
      [{ height: `${String(from)}px` }, { height: `${String(to)}px` }],
      {
        duration: fluentMotion.duration,
        easing: fluentMotion.easing,
        fill: 'both',
      },
    );
    this.#animation = animation;
    publishDisclosureHeight(this.#target, to);
    void animation.finished.then(
      () => {
        if (this.#animation !== animation) return;
        finish();
      },
      () => {
        // Reversal and disposal deliberately cancel the previous native animation.
      },
    );
  }

  /** Retain the latest requested native state when changing views interrupts presentation. */
  dispose(): void {
    this.#revision++;
    if (this.#desired !== null) this.#apply?.(this.#desired);
    this.#animation?.cancel();
    this.#animation = null;
    this.#release();
  }

  /** Restore only the transient style owned by this disclosure lifetime. */
  #release(): void {
    this.#animation?.cancel();
    this.#animation = null;
    if (this.#height !== null) {
      this.#target.style.setProperty('height', this.#height.value, this.#height.priority);
      this.#height = null;
    }
    if (this.#overflow !== null) this.#target.style.overflow = this.#overflow;
    this.#overflow = null;
    this.#desired = null;
    this.#apply = null;
    this.#owner = null;
    delete this.#target.dataset.scDisclosing;
    delete this.#target.dataset.scDisclosureTarget;
    delete this.#target.dataset.scDisclosureLayout;
    publishDisclosureHeight(this.#target, null);
  }
}

/** Read layout height in CSS units so graph camera zoom never distorts disclosure sizing. */
function height(element: HTMLElement): number {
  const computed = element.ownerDocument.defaultView?.getComputedStyle(element).height;
  const value = Number.parseFloat(computed || '');
  return Number.isFinite(value) ? value : element.offsetHeight;
}
