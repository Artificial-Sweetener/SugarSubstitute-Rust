import { nativeCanvasBackdrop } from './NativeCanvasCapabilities.js';
/** Require accepted display allocation/material, independently of newer input-routing requests. */
export function nativeMaterialReady(binding, shown, geometry) {
    return (binding !== null &&
        nativeCanvasBackdrop(binding.host) &&
        binding.committed &&
        shown &&
        geometry?.visible === true &&
        !!geometry.backdrops?.length &&
        materialIdentity(binding.lastGeometry) === materialIdentity(geometry));
}
/** Preserve drawable allocation while withholding visibility for a prepared empty presentation. */
export function nativeMaterialGeometry(binding, desired, observingPreview) {
    const geometry = desired.geometry ?? binding.lastGeometry;
    if (!binding.prepared || !geometry)
        return null;
    return desired.shown || (desired.previewable && observingPreview)
        ? geometry
        : { ...geometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
}
/** Preserve acknowledged pixels through source replacement and same-content allocation changes. */
export function nativeMaterialRetention(options) {
    const { binding, desired, observing, shown, previousContent } = options;
    const geometryMatches = binding?.lastGeometry != null &&
        desired.geometry !== null &&
        materialIdentity(binding.lastGeometry) === materialIdentity(desired.geometry);
    const presentsPixels = desired.shown || (desired.previewable && observing);
    return {
        retainedFinal: desired.previewable && observing && binding?.selectionRevision !== null && geometryMatches,
        // The host re-presents committed content at a new allocation without losing its shown frame.
        resized: shown &&
            previousContent === desired.content &&
            binding?.committed === true &&
            desired.presentable &&
            presentsPixels,
        retainPixels: presentsPixels && binding?.committed === true && geometryMatches,
    };
}
/** Identify the pixels and rounded material of an allocation, excluding gesture-routing state. */
function materialIdentity(geometry) {
    if (!geometry)
        return null;
    return JSON.stringify([
        geometry.coordinateSpace,
        geometry.units,
        geometry.origin,
        geometry.viewport,
        geometry.clip,
        geometry.physicalPixelsPerCssPixel,
        geometry.visible,
        geometry.backdrops ?? [],
    ]);
}
