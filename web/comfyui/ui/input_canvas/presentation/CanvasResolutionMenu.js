import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
/** Contribute the entry only when a targeted field is an authority of a provisioned surface. */
export function canvasResolutionEntries(targets, resolution, dialog, report) {
    const surface = targets
        .map((target) => resolution.surface(target.nodeId, target.inputName))
        .find((item) => item !== null) ?? null;
    if (!surface)
        return [];
    return [
        { label: '', separator: true },
        {
            label: WORKFLOW_SURFACE_COPY.changeCanvasResolutionMenu,
            select: () => openResolution(surface, resolution, dialog, report),
        },
    ];
}
/** Contribute the entry to the node context menu of a node that sizes a mask-only surface. */
export function canvasResolutionNodeMenuItems(node, resolution, dialog, report) {
    const surface = node === null ? null : resolution.surfaceSizedBy(node);
    if (!surface)
        return [];
    return [
        {
            content: WORKFLOW_SURFACE_COPY.changeCanvasResolutionMenu,
            callback: () => openResolution(surface, resolution, dialog, report),
        },
    ];
}
/** Open the dialog at the surface's current size and resize it on apply. */
function openResolution(surface, resolution, dialog, report) {
    dialog
        .open({ width: surface.document.width, height: surface.document.height }, (width, height, mode) => resolution.resize(surface.key, width, height, mode))
        .catch(report);
}
