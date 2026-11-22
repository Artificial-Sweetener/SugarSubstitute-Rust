import { captureMasonryScroll, masonryScrollTop } from './MasonryCubeSections.js';
/** Input by which a person takes the masonry's scroll over. */
const TAKEOVER = ['wheel', 'pointerdown', 'keydown', 'touchstart'];
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
    element;
    memory;
    report;
    #entered;
    #pending = null;
    #settled = false;
    /** Where the last application left the masonry, so its own scroll event is not a takeover. */
    #applied = null;
    /** Watch one mounted masonry element for the workflow `memory` belongs to. */
    constructor(element, memory, report) {
        this.element = element;
        this.memory = memory;
        this.report = report;
        this.#entered = element.scrollTop;
        element.addEventListener('scrollend', this.#record);
        element.addEventListener('scroll', this.#moved, { passive: true });
        element.addEventListener('sc-masonry-layout', this.#retry);
        for (const name of TAKEOVER)
            element.addEventListener(name, this.#takeOver, { passive: true });
    }
    /** The cards have entered: bring back the kept scroll unless something scrolled meanwhile. */
    settle() {
        this.#pending = this.element.scrollTop === this.#entered ? (this.memory?.read() ?? null) : null;
        this.#apply();
    }
    /** Record where the masonry rests, if settled, and stop watching it. */
    dispose() {
        this.element.removeEventListener('scrollend', this.#record);
        this.element.removeEventListener('scroll', this.#moved);
        this.element.removeEventListener('sc-masonry-layout', this.#retry);
        for (const name of TAKEOVER)
            this.element.removeEventListener(name, this.#takeOver);
        this.#record();
    }
    #apply() {
        const kept = this.#pending;
        if (kept) {
            const top = masonryScrollTop(this.element, kept);
            if (top !== null)
                this.element.scrollTop = top;
            if (top === null || Math.abs(this.element.scrollTop - top) > 1) {
                this.#applied = this.element.scrollTop;
                return;
            }
        }
        this.#pending = null;
        this.#settled = true;
        this.#record();
    }
    #retry = () => {
        if (this.#pending)
            this.#apply();
    };
    #takeOver = () => {
        if (!this.#pending)
            return;
        this.#pending = null;
        this.#settled = true;
    };
    #moved = () => {
        if (this.#pending && this.element.scrollTop !== this.#applied)
            this.#takeOver();
    };
    #record = () => {
        // A masonry taken out of the document or hidden reads as scrolled to its top; only one that
        // is laid out tells where it rests.
        if (!this.#settled || !this.memory || !this.element.isConnected || !this.element.clientHeight)
            return;
        try {
            this.memory.record(captureMasonryScroll(this.element));
        }
        catch (error) {
            this.report(error);
        }
    };
}
