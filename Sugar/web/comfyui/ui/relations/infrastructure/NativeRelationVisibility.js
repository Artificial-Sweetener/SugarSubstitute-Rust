import { isRecord } from '../../types/common.js';
import { NativeWidgetPresentation } from '../../surface/NativeWidgetPresentation.js';
/** Hide fields represented by the toolbar or a linked node while retaining native value owners. */
export class NativeRelationVisibility {
    #presentation = new NativeWidgetPresentation();
    #hiddenFields = new Map();
    /** Reconcile resolved ownership rather than inspecting card DOM or workflow implementation leaves. */
    reconcile(snapshot, inventory) {
        const hidden = new Set();
        const fields = new Map();
        for (const item of snapshot.resolved) {
            if (!item.override && !(item.source && snapshot.state.nodeLinks[item.field.nodeId]))
                continue;
            const owner = inventory.owners.get(item.field.id);
            if (!owner)
                continue;
            fields.set(item.field.id, item.field.nodeId);
            hidden.add(owner.widget);
            const faceWidget = owner.face.widgets?.find((widget) => widget.name === item.field.name);
            if (faceWidget)
                hidden.add(faceWidget);
            if (Array.isArray(owner.widget.linkedWidgets))
                for (const widget of owner.widget.linkedWidgets)
                    if (isWidget(widget))
                        hidden.add(widget);
        }
        const changed = new Set();
        for (const [id, nodeId] of this.#hiddenFields)
            if (!fields.has(id))
                changed.add(nodeId);
        for (const [id, nodeId] of fields)
            if (!this.#hiddenFields.has(id))
                changed.add(nodeId);
        this.#presentation.reconcile(hidden);
        this.#hiddenFields = fields;
        return [...changed];
    }
    /** Release all leases before a different workflow is configured. */
    dispose() {
        this.#presentation.dispose();
        this.#hiddenFields.clear();
    }
}
/** Validate linked native control identities before giving the renderer a visibility lease. */
function isWidget(value) {
    return isRecord(value) && typeof value.name === 'string' && 'value' in value;
}
