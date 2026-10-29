/** Rebuild a staged attachment when discovery changes its immutable command capabilities. */
export function nativeCanvasRendererKey(host) {
    return `native:${JSON.stringify(host?.capabilities ?? null)}`;
}
/** Advertise commands only with the viewport-state contract required by their shared proxy. */
export function nativeCanvasViewportCommands(host) {
    return host?.capabilities.viewportState ? host.capabilities.viewportCommands : [];
}
