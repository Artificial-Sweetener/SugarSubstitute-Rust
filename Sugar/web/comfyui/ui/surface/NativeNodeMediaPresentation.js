const attribute = 'data-sc-native-media';
const owners = new WeakMap();
/** Read the nearest projected media replacement without discovering nodes from DOM state. */
export function isNativeNodeMediaPresented(element) {
    return element.closest(`[${attribute}="replaced"]`) === null;
}
/** Own a reversible media presentation lease for one identified native card. */
export class NativeNodeMediaReplacement {
    root;
    #previous;
    /** Suppress Comfy's source-file media only inside the owning card's replacement surface. */
    constructor(root) {
        this.root = root;
        if (owners.has(root))
            throw new Error('Native card media has competing presentation owners.');
        owners.set(root, this);
        this.#previous = root.getAttribute(attribute);
        root.setAttribute(attribute, 'replaced');
        const document = root.ownerDocument;
        if (!document.getElementById('sc-native-media-presentation')) {
            const style = document.createElement('style');
            style.id = 'sc-native-media-presentation';
            style.textContent = `[${attribute}="replaced"] .lg-node-content{display:none!important;}`;
            document.head.append(style);
        }
    }
    /** Restore only the exact media presentation lease still owned by this lifetime. */
    dispose() {
        if (owners.get(this.root) !== this)
            return;
        owners.delete(this.root);
        if (this.#previous === null)
            this.root.removeAttribute(attribute);
        else
            this.root.setAttribute(attribute, this.#previous);
    }
}
/** Hide legacy media only during measurement and drawing, restoring exact graph property ownership. */
export function withNativeNodeMediaPresentation(element, node, draw) {
    if (isNativeNodeMediaPresented(element))
        return draw();
    const keys = ['imgs', 'animatedImages'];
    const remembered = keys.map((key) => ({
        key,
        value: Reflect.get(node, key),
        owned: Object.hasOwn(node, key),
    }));
    for (const key of keys)
        Reflect.set(node, key, []);
    try {
        return draw();
    }
    finally {
        for (const { key, value, owned } of remembered) {
            Reflect.set(node, key, value);
            if (!owned)
                Reflect.deleteProperty(node, key);
        }
    }
}
