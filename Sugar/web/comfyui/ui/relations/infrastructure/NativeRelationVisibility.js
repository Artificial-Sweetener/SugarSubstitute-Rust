import { isRecord } from '../../types/common.js';
import { NativeWidgetPresentation } from '../../surface/NativeWidgetPresentation.js';
import { presentsLinkedFields } from '../domain/RelationResolver.js';
/**
 * Hide fields represented by the toolbar or a linked node while retaining native value owners.
 *
 * A field the toolbar owns has moved off its card. A field a link owns is replaced on its card by
 * the link choice in the card's title, so the card stays. A linked prompt keeps its field.
 */
export class NativeRelationVisibility {
    #overridden = new NativeWidgetPresentation();
    #linked = new NativeWidgetPresentation('replacement');
    #hiddenFields = new Map();
    /** Reconcile resolved ownership rather than inspecting card DOM or workflow implementation leaves. */
    reconcile(snapshot, inventory) {
        const overridden = new Set();
        const linked = new Set();
        const fields = new Map();
        const nodes = new Map(snapshot.nodes.map((node) => [node.id, node]));
        for (const item of snapshot.resolved) {
            const followsLink = item.source !== null && Boolean(snapshot.state.nodeLinks[item.field.nodeId]);
            if (!item.override && !followsLink)
                continue;
            if (!item.override && presentsLinkedFields(nodes.get(item.field.nodeId)))
                continue;
            const owner = inventory.owners.get(item.field.id);
            if (!owner)
                continue;
            const hidden = item.override ? overridden : linked;
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
        // A field passing from one owner to the other is released before it is taken.
        this.#linked.release(linked);
        this.#overridden.release(overridden);
        this.#overridden.reconcile(overridden);
        this.#linked.reconcile(linked);
        this.#hiddenFields = fields;
        return [...changed];
    }
    /** Release all leases before a different workflow is configured. */
    dispose() {
        this.#overridden.dispose();
        this.#linked.dispose();
        this.#hiddenFields.clear();
    }
}
/** Validate linked native control identities before giving the renderer a visibility lease. */
function isWidget(value) {
    return isRecord(value) && typeof value.name === 'string' && 'value' in value;
}
