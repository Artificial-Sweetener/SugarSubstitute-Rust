import { isRecord } from '../types/common.js';
const eventName = 'sugarcubes-native-node-card-change';
/** Refresh copied Vue field data without replacing a card's node or interaction identity. */
export function refreshNativeNodeCards(document, change) {
    document.dispatchEvent(new CustomEvent(eventName, { detail: change }));
}
/** Scope native-data invalidation to the projected card's exact mount lifetime. */
export function subscribeNativeNodeCardRefresh(document, nodes, refresh) {
    const listener = (event) => {
        if (!(event instanceof CustomEvent))
            return;
        const change = event.detail;
        if (!isRecord(change))
            return;
        if (change.kind === 'reset' && change.reason === 'native-choices')
            refresh();
        else if (change.kind === 'nodes' &&
            Array.isArray(change.nodes) &&
            change.nodes.some((node) => nodes.some((owner) => owner === node)))
            refresh();
    };
    document.addEventListener(eventName, listener);
    return () => document.removeEventListener(eventName, listener);
}
