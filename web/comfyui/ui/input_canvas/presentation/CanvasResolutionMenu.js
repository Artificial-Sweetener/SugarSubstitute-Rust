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
            select: () => {
                dialog
                    .open({ width: surface.document.width, height: surface.document.height }, (width, height, mode) => resolution.resize(surface.key, width, height, mode))
                    .catch(report);
            },
        },
    ];
}
