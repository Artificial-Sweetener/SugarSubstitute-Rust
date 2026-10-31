import { negotiatesCanvasSurfaceComparison } from '../../core/embedded/CanvasRuntime.js';
const CAMERA_COMMANDS = ['fit', 'native', 'zoom', 'pan'];
const COMPARISON_COMMANDS = ['comparison', 'select-comparison', 'swap', 'wipe'];
/** Rebuild a staged attachment when discovery changes its immutable command capabilities. */
export function nativeCanvasRendererKey(host) {
    return `native:${JSON.stringify(host?.capabilities ?? null)}`;
}
/** Report whether the host can present an A/B pair and place its divider. */
export function nativeCanvasComparison(host) {
    return host !== null && negotiatesCanvasSurfaceComparison(host.capabilities);
}
/**
 * Advertise camera commands only with the viewport-state contract required by their shared
 * proxy, and the shared comparison commands only with a negotiated pair-and-divider host.
 */
export function nativeCanvasCommands(host) {
    const camera = host?.capabilities.viewportState
        ? host.capabilities.viewportCommands.filter((command) => CAMERA_COMMANDS.some((name) => name === command))
        : [];
    return [...camera, ...(nativeCanvasComparison(host) ? COMPARISON_COMMANDS : [])];
}
