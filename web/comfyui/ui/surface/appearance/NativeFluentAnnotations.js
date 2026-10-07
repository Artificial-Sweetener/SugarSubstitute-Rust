/** Own removable appearance properties independently of native component discovery. */
export class NativeFluentAnnotations {
    #roots = new Map();
    /** Begin one scoped reconciliation while releasing detached native controls. */
    begin(root) {
        this.prune();
        const previous = this.#roots.get(root) ?? new Map();
        return new NativeFluentAnnotationPlan(previous, (next) => this.#roots.set(root, next));
    }
    /** Release retired presentation roots even when the batch contains no surviving native cards. */
    prune() {
        for (const [element, annotations] of this.#roots)
            if (!element.isConnected) {
                clearRemoved(annotations, new Map());
                this.#roots.delete(element);
            }
    }
    /** Release exactly the properties still owned by the active appearance. */
    clear() {
        for (const annotations of this.#roots.values())
            clearRemoved(annotations, new Map());
        this.#roots.clear();
    }
}
/** Collect one native boundary's ownership and write only materially changed attributes or styles. */
export class NativeFluentAnnotationPlan {
    #next = new Map();
    #previous;
    #commit;
    /** Retain prior ownership until native discovery has identified the complete next boundary. */
    constructor(previous, commit) {
        this.#previous = previous;
        this.#commit = commit;
    }
    /** Mark one native boundary without generating redundant attribute mutations. */
    attribute(element, name, value = '') {
        this.#record(element, name, value);
        if (element.getAttribute(name) !== value)
            element.setAttribute(name, value);
    }
    /** Retain native row placement while avoiding repeated style invalidation. */
    style(element, name, value) {
        this.#record(element, name, value);
        if (element.style.getPropertyValue(name) !== value)
            element.style.setProperty(name, value);
    }
    /** Remove disappeared ownership after discovery, retaining unchanged native control identity. */
    commit() {
        clearRemoved(this.#previous, this.#next);
        this.#commit(this.#next);
    }
    /** Record the exact property so release cannot affect other appearance owners. */
    #record(element, name, value) {
        let properties = this.#next.get(element);
        if (!properties)
            this.#next.set(element, (properties = new Map()));
        properties.set(name, value);
    }
}
/** Release stale owned values while preserving subsequent native or custom changes. */
function clearRemoved(previous, next) {
    for (const [element, properties] of previous)
        for (const [name, value] of properties) {
            if (next.get(element)?.has(name))
                continue;
            if (name.startsWith('--')) {
                if (element.style.getPropertyValue(name) === value)
                    element.style.removeProperty(name);
            }
            else if (element.getAttribute(name) === value)
                element.removeAttribute(name);
        }
}
