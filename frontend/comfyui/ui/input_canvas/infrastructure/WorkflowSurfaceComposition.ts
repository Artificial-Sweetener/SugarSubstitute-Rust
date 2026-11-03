//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Compose workflow input surfaces, their route and their card interactions. */
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type { CanvasRoutePort } from '../../output_canvas/domain/CanvasRoute.js';
import type { ComfyGraph } from '../../types/graph.js';
import { WorkflowInputRoute } from '../application/WorkflowInputRoute.js';
import { WorkflowMaskAppearance } from '../application/WorkflowMaskAppearance.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
import {
  WorkflowSurfaceProvisioning,
  type WorkflowSurfaceNotice,
} from '../application/WorkflowSurfaceProvisioning.js';
import { BrowserWorkflowFileDimensions } from './BrowserWorkflowFileDimensions.js';
import type { ComfyInputAssociations } from './ComfyInputAssociations.js';
import type { ComfyInputBindings } from './ComfyInputBindings.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { ComfyInputSurfaceCards } from './ComfyInputSurfaceCards.js';
import { ComfyWorkflowInputState } from './ComfyWorkflowInputState.js';
import { ComfyWorkflowSurfaceGraph } from './ComfyWorkflowSurfaceGraph.js';

/** Host collaborators of workflow input surfaces. */
export interface WorkflowSurfaceOptions {
  readonly document: Document;
  readonly associations: ComfyInputAssociations;
  readonly bindings: ComfyInputBindings;
  readonly loaders: ComfyInputLoaders;
  graph(): ComfyGraph | null | undefined;
  workflowHost(): CanvasWorkflowHost | null;
  accent(): string;
  notice: WorkflowSurfaceNotice;
  report(error: unknown): void;
}

/** Owners of workflow input surfaces and their host-facing adapters. */
export interface WorkflowSurfaces {
  readonly provisioning: WorkflowSurfaceProvisioning;
  readonly route: CanvasRoutePort;
  readonly cards: ComfyInputSurfaceCards;
  dispose(): void;
}

/** Build provisioning over the existing association and binding owners. */
export function createWorkflowSurfaces(options: WorkflowSurfaceOptions): WorkflowSurfaces {
  const { associations, workflowHost, report } = options;
  const state = new ComfyWorkflowInputState(options.graph, () => crypto.randomUUID());
  const graph = new ComfyWorkflowSurfaceGraph(
    associations,
    options.bindings,
    options.loaders,
    state,
    report,
  );
  const provisioning = new WorkflowSurfaceProvisioning(
    graph,
    workflowHost,
    options.accent,
    new BrowserWorkflowFileDimensions(options.document),
    options.notice,
    report,
  );
  const route = new WorkflowInputRoute(provisioning, workflowHost, state, report);
  const appearance = new WorkflowMaskAppearance(provisioning, workflowHost, graph, report);
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
    cards: new ComfyInputSurfaceCards(associations, route, appearance, provisioning),
    dispose: () => {
      route.dispose();
      provisioning.dispose();
      graph.dispose();
    },
  };
}
