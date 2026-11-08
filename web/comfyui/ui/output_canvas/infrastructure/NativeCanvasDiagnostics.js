import { nativeCanvasBackdrop } from './NativeCanvasCapabilities.js';
import { nativeMaterialReady } from './NativeCanvasMaterial.js';
import { IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS, } from './NativeCanvasPreview.js';
/** Record the native offer an attachment was constructed against, for later material diagnostics. */
export function nativeConstructorSurface(host) {
    return Object.freeze({
        available: host !== null,
        schema: host?.capabilities.schema ?? null,
        backdrop: nativeCanvasBackdrop(host),
    });
}
/** Copy only the bounded material allocation; routing and cancellation objects stay private. */
export function nativeMaterialGeometrySnapshot(geometry) {
    return (geometry &&
        Object.freeze({
            coordinateSpace: geometry.coordinateSpace,
            units: geometry.units,
            origin: Object.freeze({ ...geometry.origin }),
            viewport: Object.freeze({ ...geometry.viewport }),
            clip: Object.freeze({ ...geometry.clip }),
            physicalPixelsPerCssPixel: geometry.physicalPixelsPerCssPixel,
            visible: geometry.visible,
            ...(geometry.backdrops
                ? {
                    backdrops: Object.freeze(geometry.backdrops.map((region) => Object.freeze({ ...region }))),
                }
                : {}),
        }));
}
/** Preserve immutable preview counters and cached material reasons without changing presentation. */
export function nativeCanvasDiagnostics(preview, binding, desired, shown) {
    const capabilities = binding?.host.capabilities;
    return Object.freeze({
        ...(preview?.diagnostics() ?? IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS),
        material: Object.freeze({
            bound: binding !== null,
            bindingCapabilities: capabilities
                ? Object.freeze({
                    ...capabilities,
                    imageFormats: Object.freeze([...capabilities.imageFormats]),
                    viewportCommands: Object.freeze([...capabilities.viewportCommands]),
                    previewFormats: Object.freeze([...capabilities.previewFormats]),
                    ...(capabilities.backdrop
                        ? { backdrop: Object.freeze({ ...capabilities.backdrop }) }
                        : {}),
                })
                : null,
            prepared: binding?.prepared ?? false,
            committed: binding?.committed ?? false,
            shown,
            desired: desired !== null,
            currentGeometry: nativeMaterialGeometrySnapshot(desired?.geometry ?? null),
            acknowledgedGeometry: nativeMaterialGeometrySnapshot(binding?.lastGeometry ?? null),
            activeOperation: binding?.active?.type ?? null,
            selectionRevision: binding?.selectionRevision ?? null,
            conditionsReady: nativeMaterialReady(binding, shown, desired?.geometry ?? null),
        }),
    });
}
