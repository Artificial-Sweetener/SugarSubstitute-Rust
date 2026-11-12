import { canvasCameraGeometry } from '../domain/CanvasCamera.js';
/** Inspect outgoing replacement pixels until their successor is ready, suspending absent geometry. */
export function inspectBrowserCanvasFrame(camera, frame, surface) {
    const geometry = surface?.visible && frame
        ? canvasCameraGeometry(frame.dimensions, surface.viewport, surface.physicalPixelsPerCssPixel)
        : null;
    if (frame && geometry) {
        camera.show(frame.selection, geometry, frame.runOrder, frame.previewLane);
        return true;
    }
    camera.suspend();
    return false;
}
