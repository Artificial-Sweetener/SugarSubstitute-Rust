import { cubeFaceUnconsumedWidgets } from './CubeFaceWidgetPolicy.js';
/** Retain advanced actions through collapse while leaving native state and history authoritative. */
export function nativeAdvancedInputControl(node, refresh) {
    if (!cubeFaceUnconsumedWidgets(node).some((widget) => widget.options?.advanced === true || widget.advanced === true))
        return null;
    return {
        shown: () => node.showAdvanced === true,
        setShown: (shown) => {
            if (shown === (node.showAdvanced === true))
                return;
            node.graph?.beforeChange?.();
            node.showAdvanced = shown;
            node.graph?.afterChange?.();
            refresh();
        },
    };
}
