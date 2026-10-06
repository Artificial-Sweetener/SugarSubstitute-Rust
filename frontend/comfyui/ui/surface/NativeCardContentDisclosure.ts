/** Animate changes to native widget membership through the card's shared disclosure owner. */
import { CardDisclosureMotion } from './motion/CardDisclosureMotion.js';
import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';

/** Separate native content invalidation from value updates and initial hydration. */
export class NativeCardContentDisclosure {
  #membership: string | null = null;

  /** Share the exact mount's motion lifetime with native collapse gestures. */
  constructor(
    private readonly target: HTMLElement,
    private readonly motion: CardDisclosureMotion,
  ) {}

  /** Render native updates immediately, animating only a changed set of presented widgets. */
  reconcile(widgets: readonly string[], render: () => void): void {
    const membership = JSON.stringify(widgets);
    const changed = this.#membership !== null && this.#membership !== membership;
    this.#membership = membership;
    if (changed) this.motion.resize(render, { settle: () => settleNativeDisclosure(this.target) });
    else render();
  }
}
