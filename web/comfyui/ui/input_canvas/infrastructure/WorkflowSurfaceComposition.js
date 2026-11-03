import { WorkflowInputRoute } from '../application/WorkflowInputRoute.js';
import { WorkflowMaskAppearance } from '../application/WorkflowMaskAppearance.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
import { WorkflowSurfaceProvisioning, } from '../application/WorkflowSurfaceProvisioning.js';
import { BrowserWorkflowFileDimensions } from './BrowserWorkflowFileDimensions.js';
import { ComfyInputSurfaceCards } from './ComfyInputSurfaceCards.js';
import { BrowserMaskImport } from './BrowserMaskImport.js';
import { WorkflowMaskImport } from '../application/WorkflowMaskImport.js';
import { ComfyWorkflowInputState } from './ComfyWorkflowInputState.js';
import { ComfyWorkflowSurfaceGraph } from './ComfyWorkflowSurfaceGraph.js';
import { ComfyDimensionAuthorityWriter } from './ComfyDimensionAuthorityWriter.js';
import { WorkflowCanvasResolution } from '../application/WorkflowCanvasResolution.js';
import { CanvasResolutionDialog } from '../presentation/CanvasResolutionDialog.js';
import { canvasResolutionEntries, } from '../presentation/CanvasResolutionMenu.js';
/** Build provisioning over the existing association and binding owners. */
export function createWorkflowSurfaces(options) {
    const { associations, workflowHost, report } = options;
    const state = new ComfyWorkflowInputState(options.graph, () => crypto.randomUUID(), options.workflowPeers, options.commitWorkflow);
    const graph = new ComfyWorkflowSurfaceGraph(associations, options.bindings, options.loaders, state, report);
    const provisioning = new WorkflowSurfaceProvisioning(graph, workflowHost, options.accent, new BrowserWorkflowFileDimensions(options.document), options.notice, report);
    const route = new WorkflowInputRoute(provisioning, workflowHost, state, report);
    const appearance = new WorkflowMaskAppearance(provisioning, workflowHost, graph, report);
    const resolution = new WorkflowCanvasResolution(provisioning, workflowHost, new ComfyDimensionAuthorityWriter((path) => associations.inventory.node(path), options.graph));
    const dialog = new CanvasResolutionDialog(options.document);
    return {
        provisioning,
        route: {
            read: () => {
                const view = route.read();
                return view
                    ? {
                        route: view.route,
                        inputAvailable: view.available,
                        reason: view.reason,
                        labels: {
                            input: WORKFLOW_SURFACE_COPY.inputRoute,
                            output: WORKFLOW_SURFACE_COPY.outputRoute,
                            group: WORKFLOW_SURFACE_COPY.routeLabel,
                        },
                    }
                    : null;
            },
            subscribe: (listener) => route.subscribe(listener),
            choose: (value) => route.chooseRoute(value),
        },
        cards: new ComfyInputSurfaceCards(associations, route, appearance, provisioning, new WorkflowMaskImport(provisioning, new BrowserMaskImport(options.document), options.assets), options.bindings),
        resolutionEntries: (targets) => canvasResolutionEntries(targets, resolution, dialog, report),
        dispose: () => {
            dialog.close();
            route.dispose();
            provisioning.dispose();
            graph.dispose();
        },
    };
}
