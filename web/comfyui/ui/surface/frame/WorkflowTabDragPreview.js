import { WORKFLOW_TAB_ORB_CUTOUT_RADIUS } from './WorkflowTabStyles.js';
const motion = { duration: 180, easing: 'cubic-bezier(.22,1,.36,1)' };
/** Retain visual continuity across the host's accepted DOM permutation. */
export class WorkflowTabDragPreview {
    list;
    viewport;
    restored;
    tabWidth;
    #animations = new Map();
    #elementAnimations = new Map();
    #active = false;
    /** Restore canonical orb destinations when accepted order is projected. */
    constructor(list, viewport, restored, tabWidth) {
        this.list = list;
        this.viewport = viewport;
        this.restored = restored;
        this.tabWidth = tabWidth;
    }
    /** Report whether the pointer preview owns geometry before accepted order is projected. */
    get active() {
        return this.#active;
    }
    /** Move the held tab immediately and animate displaced neighbors from stable slots. */
    show(tabs, origin, placement) {
        if (!this.#active) {
            this.#captureBackdrop();
            this.#cancelElement(this.list);
        }
        this.#active = true;
        this.viewport.setAttribute('data-sc-tab-dragging', '');
        const first = Math.floor(this.viewport.scrollLeft / this.tabWidth());
        const cutoutDisabled = this.viewport.hasAttribute('data-sc-overflow') ||
            this.viewport.hasAttribute('data-sc-orb-hidden');
        const progress = cutoutDisabled
            ? 0
            : Math.max(0, Math.min(1, 1 - (placement.draggedLeft / this.tabWidth() - first)));
        const successor = cutoutDisabled ? -1 : origin === first ? first + 1 : first;
        for (const [index, tab] of tabs.entries()) {
            const offset = placement.offsets[index] ?? 0;
            const destination = `translateX(${String(offset)}px)`;
            if (index === origin) {
                this.#cancelElement(tab);
                tab.style.transition = 'none';
                tab.style.transform = destination;
            }
            else if (tab.style.transform !== destination) {
                const before = tab.getBoundingClientRect().left;
                this.#cancelElement(tab);
                tab.style.transition = 'none';
                tab.style.transform = destination;
                const from = offset + before - tab.getBoundingClientRect().left;
                if (from !== offset && !this.#reducedMotion() && typeof tab.animate === 'function')
                    this.#animate(tab, [{ transform: `translateX(${String(from)}px)` }, { transform: destination }], undefined, 150);
            }
            tab.toggleAttribute('data-sc-held', index === origin);
            const cutout = index === origin ? progress : index === successor ? 1 - progress : 0;
            tab.style.paddingLeft = `${String(20 + 12 * cutout)}px`;
            tab.style.setProperty('--sc-tab-cutout-radius', `${String(WORKFLOW_TAB_ORB_CUTOUT_RADIUS * cutout)}px`);
            tab.style.setProperty('--sc-tab-cutout-x', `${String(-17 * cutout)}px`);
        }
    }
    /** Composite the translucent held painter against its frame, obscuring covered sibling text. */
    #captureBackdrop() {
        const window = this.list.ownerDocument.defaultView;
        if (!window)
            return;
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
    capture(tabs) {
        return new Map(tabs.map((tab) => [tab, tab.getBoundingClientRect().left]));
    }
    /** Animate from the currently painted position into each accepted slot without a jump. */
    settle(tabs, positions) {
        this.#cancelAnimations();
        this.viewport.removeAttribute('data-sc-tab-dragging');
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
            const destinationRadius = tab.hasAttribute('data-orb-adjacent')
                ? `${String(WORKFLOW_TAB_ORB_CUTOUT_RADIUS)}px`
                : '0px';
            const destinationX = tab.style.getPropertyValue('--sc-tab-cutout-x') || '-17px';
            const destinationPadding = tab.hasAttribute('data-orb-adjacent') ? '32px' : '20px';
            const finished = () => {
                tab.removeAttribute('data-sc-held');
                tab.style.removeProperty('transition');
            };
            if ((delta || radius !== destinationRadius) &&
                !this.#reducedMotion() &&
                typeof tab.animate === 'function')
                this.#animate(tab, [
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
                ], finished);
            else
                finished();
        }
    }
    /** Track native animations without retaining rejected completion promises. */
    #animate(element, frames, finished, duration = motion.duration) {
        const animation = element.animate(frames, { ...motion, duration });
        const cleanup = () => {
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
    #cancelElement(element) {
        const animation = this.#elementAnimations.get(element);
        if (!animation)
            return;
        const cleanup = this.#animations.get(animation);
        animation.onfinish = animation.oncancel = null;
        animation.cancel();
        cleanup?.();
    }
    /** Respect the host browser's accessibility preference at the animation boundary. */
    #reducedMotion() {
        return (this.list.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
            .matches ?? false);
    }
    /** Retire outstanding motion before a new preview or unmount. */
    #cancelAnimations() {
        for (const [animation, cleanup] of this.#animations) {
            animation.onfinish = animation.oncancel = null;
            animation.cancel();
            cleanup();
        }
        this.#animations.clear();
    }
    /** Release native animation resources when the frame is disconnected. */
    dispose() {
        this.#cancelAnimations();
    }
}
