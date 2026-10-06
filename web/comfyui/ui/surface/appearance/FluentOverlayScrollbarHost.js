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
/** Own global discovery and the reversible lifetime of Fluent overlay scrollbars. */
import { fluentNativeScrollbarCss } from './FluentScrollbarStyles.js';
import { scrollbarTrack } from './FluentScrollbarLayout.js';
import { FluentScrollbarView } from './FluentScrollbarView.js';
/** Observe native overflow while leaving the host's layout and scrolling intact. */
export class FluentOverlayScrollbarHost {
    #document;
    #reportError;
    #layer;
    #owners = new Map();
    #views = new Map();
    #roots = new Set();
    #shadowStyles = new Map();
    #pending = new Set();
    #observedSizes = new Set();
    #mutations;
    #resize;
    #active = false;
    #frame = null;
    #nextId = 0;
    /** Prepare a single portalled layer and coalesced observers. */
    constructor(document, reportError) {
        this.#document = document;
        this.#reportError = reportError;
        this.#layer = document.createElement('div');
        this.#layer.className = 'sc-fluent-scrollbar-layer';
        this.#mutations = new MutationObserver((records) => {
            for (const record of records) {
                if (record.target instanceof HTMLElement && !this.#owned(record.target))
                    this.#pending.add(record.target);
                for (const node of record.addedNodes)
                    if (node instanceof HTMLElement && !this.#owned(node))
                        this.#pending.add(node);
            }
            if (this.#pending.size)
                this.#schedule();
        });
        this.#resize =
            typeof ResizeObserver === 'function' ? new ResizeObserver(() => this.#schedule()) : null;
    }
    /** Start the policy once, including existing host dialogs and open shadow surfaces. */
    enable() {
        if (this.#active)
            return;
        this.#active = true;
        this.#document.body.append(this.#layer);
        this.#observe(this.#document.body);
        this.#pending.add(this.#document.body);
        this.#document.addEventListener('scroll', this.#schedule, true);
        this.#document.addEventListener('input', this.#schedule, true);
        this.#document.defaultView?.addEventListener('resize', this.#schedule);
        this.#document.defaultView?.addEventListener('pointerdown', this.#captureThumb, true);
        this.#schedule();
    }
    /** Restore native presentation and remove only IDs and styles owned by this host. */
    clear() {
        this.#active = false;
        if (this.#frame !== null)
            this.#document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.#mutations.disconnect();
        this.#resize?.disconnect();
        this.#observedSizes.clear();
        this.#document.removeEventListener('scroll', this.#schedule, true);
        this.#document.removeEventListener('input', this.#schedule, true);
        this.#document.defaultView?.removeEventListener('resize', this.#schedule);
        this.#document.defaultView?.removeEventListener('pointerdown', this.#captureThumb, true);
        for (const [element, owner] of this.#owners)
            this.#remove(element, owner);
        this.#owners.clear();
        this.#views.clear();
        for (const [root, style] of this.#shadowStyles) {
            style.remove();
            root.removeEventListener('scroll', this.#schedule, true);
        }
        this.#shadowStyles.clear();
        this.#roots.clear();
        this.#pending.clear();
        this.#layer.remove();
    }
    /** Exclude our own portal and policy styles from rediscovery. */
    #owned(element) {
        return this.#layer.contains(element) || element.hasAttribute('data-sc-scrollbar-policy');
    }
    /** Subscribe once per light or open shadow tree. */
    #observe(root) {
        if (this.#roots.has(root))
            return;
        this.#roots.add(root);
        this.#mutations.observe(root, {
            childList: true,
            subtree: true,
            attributes: true,
            attributeFilter: ['class', 'style', 'hidden'],
        });
    }
    /** Discover changed subtrees rather than walking the entire document on every scroll. */
    #discover(element) {
        if (this.#owned(element))
            return;
        this.#consider(element);
        for (const child of element.querySelectorAll('*')) {
            if (child instanceof HTMLElement && !this.#owned(child))
                this.#consider(child);
        }
    }
    /** Register potential overflow even before asynchronously loaded content fills it. */
    #consider(element) {
        const style = this.#document.defaultView?.getComputedStyle(element);
        const scrollable = element === this.#document.scrollingElement ||
            element.tagName === 'TEXTAREA' ||
            /auto|scroll/.test(`${style?.overflow} ${style?.overflowX} ${style?.overflowY}`);
        if (scrollable && !this.#owners.has(element)) {
            const assignedId = element.id ? null : `sc-fluent-scroll-owner-${++this.#nextId}`;
            if (assignedId)
                element.id = assignedId;
            const vertical = new FluentScrollbarView(element, 'vertical', element.id, this.#schedule);
            const horizontal = new FluentScrollbarView(element, 'horizontal', element.id, this.#schedule);
            this.#owners.set(element, { vertical, horizontal, assignedId });
            this.#views.set(vertical.element, vertical);
            this.#views.set(horizontal.element, horizontal);
            this.#layer.append(vertical.element, horizontal.element);
            this.#observeSize(element);
        }
        if (scrollable)
            for (const child of element.children)
                if (child instanceof HTMLElement)
                    this.#observeSize(child);
        const root = element.shadowRoot;
        if (root && !this.#shadowStyles.has(root)) {
            const styleElement = this.#document.createElement('style');
            styleElement.setAttribute('data-sc-scrollbar-policy', '');
            styleElement.textContent = fluentNativeScrollbarCss();
            root.append(styleElement);
            this.#shadowStyles.set(root, styleElement);
            root.addEventListener('scroll', this.#schedule, true);
            this.#observe(root);
        }
        if (root)
            for (const child of root.children)
                if (child instanceof HTMLElement)
                    this.#discover(child);
    }
    /** Tear down a detached owner without restoring IDs subsequently changed by the host. */
    #remove(element, owner) {
        this.#views.delete(owner.vertical.element);
        this.#views.delete(owner.horizontal.element);
        owner.vertical.dispose();
        owner.horizontal.dispose();
        this.#resize?.unobserve(element);
        this.#observedSizes.delete(element);
        for (const child of element.children)
            if (child instanceof HTMLElement)
                this.#resize?.unobserve(child);
        if (owner.assignedId && element.id === owner.assignedId)
            element.removeAttribute('id');
    }
    /** Route owned thumb presses before document-level popup dismissal and graph handlers. */
    #captureThumb = (event) => {
        if (!(event.target instanceof HTMLElement))
            return;
        const view = this.#views.get(event.target);
        if (!view || event.button !== 0)
            return;
        view.beginDrag(event);
        event.stopImmediatePropagation();
    };
    /** Subscribe once per size owner so repeated discovery cannot retrigger resize delivery. */
    #observeSize(element) {
        if (!this.#resize || this.#observedSizes.has(element))
            return;
        this.#observedSizes.add(element);
        this.#resize.observe(element);
    }
    /** Release removed content and shadow trees instead of retaining them until theme shutdown. */
    #prune() {
        for (const element of this.#observedSizes) {
            if (element.isConnected)
                continue;
            this.#resize?.unobserve(element);
            this.#observedSizes.delete(element);
        }
        let rootsChanged = false;
        for (const [root, style] of this.#shadowStyles) {
            if (root.host.isConnected)
                continue;
            style.remove();
            root.removeEventListener('scroll', this.#schedule, true);
            this.#shadowStyles.delete(root);
            this.#roots.delete(root);
            rootsChanged = true;
        }
        if (rootsChanged) {
            this.#mutations.disconnect();
            const roots = [...this.#roots];
            this.#roots.clear();
            for (const root of roots)
                this.#observe(root);
        }
    }
    /** Batch geometry reads and fail visibly through the existing diagnostic owner. */
    #schedule = () => {
        if (!this.#active || this.#frame !== null)
            return;
        const run = () => {
            this.#frame = null;
            if (!this.#active)
                return;
            try {
                this.#prune();
                const pending = [...this.#pending];
                this.#pending.clear();
                for (const element of pending)
                    if (element.isConnected)
                        this.#discover(element);
                const root = this.#document.scrollingElement;
                if (root instanceof HTMLElement)
                    this.#consider(root);
                for (const [element, owner] of this.#owners) {
                    if (!element.isConnected) {
                        this.#remove(element, owner);
                        this.#owners.delete(element);
                        continue;
                    }
                    const track = scrollbarTrack(element);
                    owner.vertical.update(track);
                    owner.horizontal.update(track);
                }
            }
            catch (error) {
                this.#reportError(error);
                this.clear();
            }
        };
        const windowRef = this.#document.defaultView;
        if (windowRef?.requestAnimationFrame)
            this.#frame = windowRef.requestAnimationFrame(run);
        else {
            this.#frame = -1;
            queueMicrotask(run);
        }
    };
}
