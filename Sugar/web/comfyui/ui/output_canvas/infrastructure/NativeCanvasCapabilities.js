import { negotiatesCanvasSurfaceComparison, negotiatesCanvasSurfaceInspection, negotiatesCanvasSurfaceBackdrop, } from '../../core/embedded/CanvasRuntime.js';
const CAMERA_COMMANDS = ['fit', 'native', 'zoom', 'pan'];
const COMPARISON_COMMANDS = ['select-comparison', 'swap', 'wipe'];
/** Rebuild a staged attachment when discovery changes its immutable command capabilities. */
export function nativeCanvasRendererKey(host) {
    return `native:${JSON.stringify(host?.capabilities ?? null)}`;
}
/** Report whether the host can present an A/B pair and place its divider. */
export function nativeCanvasComparison(host) {
    return host !== null && negotiatesCanvasSurfaceComparison(host.capabilities);
}
/** Report whether the host can paint the shared overview grid's cells. */
export function nativeCanvasOverview(host) {
    return host?.capabilities.overview === true;
}
/** Report whether the host routes native canvas gestures against page-published occlusions. */
export function nativeCanvasInput(host) {
    return host?.capabilities.input === true;
}
/** Admit native-frame material only through the negotiated geometry/frame acknowledgement contract. */
export function nativeCanvasBackdrop(host) {
    return host !== null && negotiatesCanvasSurfaceBackdrop(host.capabilities);
}
/**
 * Advertise camera commands only with the viewport-state contract required by their shared
 * proxy, pair controls only with a negotiated pair-and-divider host, and the
 * overview grid only with a host that paints it. Linked inspection requires a host that
 * restores image-relative cameras. Comparison exit remains available during capability loss;
 * shared action admission separately checks whether comparison can be entered.
 */
export function nativeCanvasCommands(host) {
    const camera = host?.capabilities.viewportState
        ? host.capabilities.viewportCommands.filter((command) => CAMERA_COMMANDS.some((name) => name === command))
        : [];
    return [
        ...camera,
        'comparison',
        ...(nativeCanvasComparison(host) ? COMPARISON_COMMANDS : []),
        ...(nativeCanvasOverview(host) ? ['overview'] : []),
        ...(nativeCanvasInspection(host) ? ['link'] : []),
    ];
}
/** Report whether the host restores image-relative cameras and reports their centers. */
export function nativeCanvasInspection(host) {
    return host !== null && negotiatesCanvasSurfaceInspection(host.capabilities);
}
