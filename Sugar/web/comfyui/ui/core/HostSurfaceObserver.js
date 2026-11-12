/** Filter host mutations at their source without promoting a portal's parent to a discovery root. */
export class HostSurfaceObserver {
    #observer;
    #process;
    /** Observe named host boundaries and report only actual mounts or explicitly requested local changes. */
    constructor(root, selector, changed, options = {}) {
        this.#process = (records) => {
            const excluded = new WeakMap();
            /** Treat explicitly owned rendering trees as opaque and share ancestry checks within each delivery. */
            const isExcluded = (element) => {
                const path = [];
                let current = element;
                let result = false;
                while (current) {
                    const cached = excluded.get(current);
                    if (cached !== undefined) {
                        result = cached;
                        break;
                    }
                    path.push(current);
                    if (current.hasAttribute('data-sc-host-opaque') ||
                        (options.exclude && current.matches(options.exclude))) {
                        result = true;
                        break;
                    }
                    current = current.parentElement;
                }
                for (const ancestor of path)
                    excluded.set(ancestor, result);
                return result;
            };
            const changes = new Map();
            const mounted = new Set();
            const removed = new Set();
            const content = new Set();
            const collect = (node, kind) => {
                if (!(node instanceof HTMLElement) || isExcluded(node))
                    return;
                if (node.matches(selector)) {
                    changes.set(node, kind);
                    (kind === 'mounted' ? mounted : removed).add(node);
                }
                if (!node.childElementCount)
                    return;
                for (const element of node.querySelectorAll(selector))
                    if (!isExcluded(element)) {
                        changes.set(element, kind);
                        (kind === 'mounted' ? mounted : removed).add(element);
                    }
            };
            for (const record of records) {
                const target = record.target instanceof Element ? record.target : record.target.parentElement;
                if (target && isExcluded(target))
                    continue;
                const surface = options.content || record.type === 'attributes'
                    ? target?.closest(selector)
                    : null;
                if (surface) {
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
