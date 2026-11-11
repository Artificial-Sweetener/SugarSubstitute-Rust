import { isRecord } from '../types/common.js';
const eventName = 'sugarcubes-native-node-card-change';
const prepareEventName = 'sugarcubes-native-node-card-prepare';
/** Refresh copied Vue field data without replacing a card's node or interaction identity. */
export function refreshNativeNodeCards(document, change) {
    document.dispatchEvent(new CustomEvent(prepareEventName, { detail: change }));
    document.dispatchEvent(new CustomEvent(eventName, { detail: change }));
}
/** Scope native-data invalidation to the projected card's exact mount lifetime. */
export function subscribeNativeNodeCardRefresh(document, nodes, refresh, prepare) {
    const listener = (event) => {
        if (matches(event, nodes))
            refresh();
    };
    const preparing = (event) => {
        if (matches(event, nodes))
            prepare?.();
    };
    if (prepare)
        document.addEventListener(prepareEventName, preparing);
    document.addEventListener(eventName, listener);
    return () => {
        if (prepare)
            document.removeEventListener(prepareEventName, preparing);
        document.removeEventListener(eventName, listener);
    };
}
/** Identify the same exact native owners for geometry preparation and data rendering. */
function matches(event, nodes) {
    if (!(event instanceof CustomEvent))
        return false;
    const change = event.detail;
    if (!isRecord(change))
        return false;
    return ((change.kind === 'reset' && change.reason === 'native-choices') ||
        (change.kind === 'nodes' &&
            Array.isArray(change.nodes) &&
            change.nodes.some((node) => nodes.some((owner) => owner === node))));
}
