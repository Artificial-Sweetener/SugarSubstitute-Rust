/** Filter host mutations at their source without promoting a portal's parent to a discovery root. */
export class HostSurfaceObserver {
    #observer;
    #process;
    /** Observe named host boundaries and report only actual mounts or explicitly requested local changes. */
    constructor(root, selector, changed, options = {}) {
        this.#process = (records) => {
            const changes = new Map();
            const mounted = new Set();
            const removed = new Set();
            const content = new Set();
            const collect = (node, kind) => {
                if (!(node instanceof HTMLElement) || node.closest(options.exclude ?? ':not(*)'))
                    return;
                if (node.matches(selector)) {
                    changes.set(node, kind);
                    (kind === 'mounted' ? mounted : removed).add(node);
                }
                for (const element of node.querySelectorAll(selector))
                    if (!element.closest(options.exclude ?? ':not(*)')) {
                        changes.set(element, kind);
                        (kind === 'mounted' ? mounted : removed).add(element);
                    }
            };
            for (const record of records) {
                const target = record.target instanceof Element ? record.target : record.target.parentElement;
                if (target?.closest(options.exclude ?? ':not(*)'))
                    continue;
                const surface = target?.closest(selector);
                if ((options.content || record.type === 'attributes') && surface) {
                    changes.set(surface, 'content');
                    content.add(surface);
                }
                for (const node of record.addedNodes)
                    collect(node, 'mounted');
                for (const node of record.removedNodes)
                    collect(node, 'removed');
            }
            if (changes.size)
                changed([...changes].map(([element, kind]) => {
                    if (mounted.has(element) && removed.has(element) && element.isConnected)
                        return {
                            element,
                            kind: content.has(element) ? 'content' : 'mounted',
                            relocated: true,
                        };
                    return { element, kind };
                }));
        };
        this.#observer = new MutationObserver(this.#process);
        this.#observer.observe(root, {
            childList: true,
            subtree: true,
            characterData: options.characterData ?? false,
            ...(options.attributes?.length
                ? { attributes: true, attributeFilter: [...options.attributes] }
                : {}),
        });
    }
    /** Deliver queued host mounts before a synchronous presentation reconciliation. */
    flushPending() {
        this.#process(this.#observer.takeRecords());
    }
    /** Discard mutations produced synchronously by the consumer's own presentation update. */
    discardPending() {
        this.#observer.takeRecords();
    }
    /** Release native observation with the host adapter's lifetime. */
    disconnect() {
        this.#observer.disconnect();
    }
}
