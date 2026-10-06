import { readComfyModelFields } from './ComfyModelFieldAdapter.js';
import { comfyModelFieldOwner } from './ComfyModelFieldOwner.js';
import { ModelPickerSession } from './ModelPickerSession.js';
/** Keep transfers attached to actual graph nodes across masonry and appearance remounts. */
export class ModelPickerSessions {
    #document;
    #snapshot;
    #app;
    #owners = new Map();
    /** Bind the current workflow and catalog used to validate legal choices at acceptance time. */
    constructor(document, snapshot, app) {
        this.#document = document;
        this.#snapshot = snapshot;
        this.#app = app;
    }
    /** Reattach a replacement presentation to the same workflow-owned interaction. */
    get(root, binding) {
        const owner = comfyModelFieldOwner(this.#app, root, binding);
        let fields = this.#owners.get(owner.identity);
        if (!fields) {
            fields = new Map();
            this.#owners.set(owner.identity, fields);
        }
        const inputName = binding.inputName ?? binding.label;
        const kind = binding.kind;
        const key = JSON.stringify([inputName, kind]);
        let field = fields.get(key);
        if (!field) {
            const roots = new Set();
            const session = new ModelPickerSession(this.#document, () => {
                if (!owner.exists())
                    return null;
                const snapshot = this.#snapshot();
                if (snapshot)
                    for (const current of roots) {
                        if (!current.isConnected)
                            continue;
                        const receiver = readComfyModelFields(current, snapshot).find((candidate) => candidate.inputName === inputName && candidate.kind === kind);
                        if (receiver)
                            return receiver;
                    }
                return owner.receiver();
            });
            field = { owner, roots, session };
            fields.set(key, field);
        }
        field.roots.add(root);
        return field.session;
    }
    /** Prune detached presentations while cancelling only deleted or replaced workflow owners. */
    reconcile() {
        for (const [identity, fields] of this.#owners) {
            for (const [key, field] of fields) {
                for (const root of field.roots)
                    if (!root.isConnected)
                        field.roots.delete(root);
                if (field.owner.exists())
                    continue;
                field.session.dispose();
                fields.delete(key);
            }
            if (!fields.size)
                this.#owners.delete(identity);
        }
    }
    /** Cancel interactions when the supported renderer or enhancement is explicitly disabled. */
    clear() {
        for (const fields of this.#owners.values())
            for (const field of fields.values())
                field.session.dispose();
        this.#owners.clear();
    }
}
