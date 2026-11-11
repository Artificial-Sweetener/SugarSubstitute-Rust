/**
 * Commit every authority's fields as one native history step, through each widget's own
 * callback and the node's change notification, as a user edit would.
 */
export class ComfyDimensionAuthorityWriter {
    node;
    root;
    /** Resolve flattened paths through the association inventory and history through the root graph. */
    constructor(node, root) {
        this.node = node;
        this.root = root;
    }
    /** Refuse before changing anything when any authority field is gone. */
    write(nodes, width, height) {
        const fields = nodes.flatMap((item) => {
            const node = this.node(item.node);
            const widthField = node?.widgets?.find((widget) => widget.name === item.width);
            const heightField = node?.widgets?.find((widget) => widget.name === item.height);
            if (!node || !widthField || !heightField)
                throw new Error('A node that sizes this canvas is no longer in the workflow.');
            return [
                [node, widthField, width],
                [node, heightField, height],
            ];
        });
        const graph = this.root();
        graph?.beforeChange?.();
        try {
            for (const [node, widget, value] of fields)
                commit(node, widget, value);
        }
        finally {
            graph?.afterChange?.();
        }
    }
}
/** Apply one value through the widget's callback and the node's widget-change hook. */
function commit(node, widget, value) {
    const previous = widget.value;
    if (previous === value)
        return;
    widget.value = value;
    widget.callback?.(value);
    if (typeof node.onWidgetChanged === 'function')
        node.onWidgetChanged.call(node, widget.name, value, previous, widget);
}
