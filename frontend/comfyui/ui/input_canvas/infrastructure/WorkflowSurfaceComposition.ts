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
import { isRecord } from '../../types/common.js';
import { WorkflowInputRoute } from '../application/WorkflowInputRoute.js';
import { WorkflowMaskAppearance } from '../application/WorkflowMaskAppearance.js';
import { WorkflowRegionalHover } from '../application/WorkflowRegionalHover.js';
import { WorkflowRegionalPrompts } from '../application/WorkflowRegionalPrompts.js';
import { ComfyRegionalPromptLinks } from './ComfyRegionalPromptLinks.js';
import { ComfyRegionalPromptNames } from './ComfyRegionalPromptNames.js';
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
import { BrowserMaskImport } from './BrowserMaskImport.js';
import { WorkflowMaskImport } from '../application/WorkflowMaskImport.js';
import type { InputSourceAssets } from '../application/InputSourcePreparation.js';
import { ComfyWorkflowInputState } from './ComfyWorkflowInputState.js';
import { ComfyWorkflowSurfaceGraph } from './ComfyWorkflowSurfaceGraph.js';
import { ComfyDimensionAuthorityWriter } from './ComfyDimensionAuthorityWriter.js';
import { WorkflowCanvasResolution } from '../application/WorkflowCanvasResolution.js';
import { CanvasResolutionDialog } from '../presentation/CanvasResolutionDialog.js';
import {
  canvasResolutionEntries,
  canvasResolutionNodeMenuItems,
  type CanvasResolutionFieldTarget,
  type CanvasResolutionNodeMenuItem,
} from '../presentation/CanvasResolutionMenu.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';

/** Host collaborators of workflow input surfaces. */
export interface WorkflowSurfaceOptions {
  readonly document: Document;
  readonly associations: ComfyInputAssociations;
  readonly bindings: ComfyInputBindings;
  readonly loaders: ComfyInputLoaders;
  /** Stages imported region masks as immutable input files. */
  readonly assets: InputSourceAssets;
  graph(): ComfyGraph | null | undefined;
  workflowHost(): CanvasWorkflowHost | null;
  /** Stored states of the other open workflows, used to recognize a duplicated workflow. */
  workflowPeers(): readonly unknown[];
  /** Record the active workflow's graph in its change tracker. */
  commitWorkflow(): void;
  accent(): string;
  /** Observe theme accent changes. */
  subscribeAccent(listener: () => void): () => void;
  notice: WorkflowSurfaceNotice;
  report(error: unknown): void;
}

/** Owners of workflow input surfaces and their host-facing adapters. */
export interface WorkflowSurfaces {
  readonly provisioning: WorkflowSurfaceProvisioning;
  /** The loaded workflow's input-canvas identity, or null when no workflow is loaded. */
  workflow(): string | null;
  readonly route: CanvasRoutePort;
  readonly cards: ComfyInputSurfaceCards;
  /** Link prompt editors' separator hover with mask rows and canvas masks. */
  readonly regions: ComfyRegionalPromptLinks;
  /** Offer "Change canvas resolution" for width/height fields that size a mask-only surface. */
  resolutionEntries(targets: readonly CanvasResolutionFieldTarget[]): ContextMenuEntry[];
  /** Append input-canvas node actions to the host's node context-menu items. */
  withNodeMenuItems<Item>(
    node: unknown,
    items: readonly Item[],
  ): (Item | CanvasResolutionNodeMenuItem)[];
  dispose(): void;
}

/** Build provisioning over the existing association and binding owners. */
export function createWorkflowSurfaces(options: WorkflowSurfaceOptions): WorkflowSurfaces {
  const { associations, workflowHost, report } = options;
  const state = new ComfyWorkflowInputState(
    options.graph,
    () => crypto.randomUUID(),
    options.workflowPeers,
    options.commitWorkflow,
  );
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
  const accent = { current: options.accent, subscribe: options.subscribeAccent };
  const regionalPrompts = new WorkflowRegionalPrompts(graph, accent, report);
  // Canvas masks take their colours from the accent; an unchanged plan is not resent.
  const stopAccent = options.subscribeAccent(() => void provisioning.request());
  const regionalHover = new WorkflowRegionalHover(regionalPrompts, appearance, report);
  const promptNames = new ComfyRegionalPromptNames(
    regionalPrompts,
    (path) => associations.inventory.node(path),
    options.graph,
    report,
  );
  const resolution = new WorkflowCanvasResolution(
    provisioning,
    workflowHost,
    new ComfyDimensionAuthorityWriter((path) => associations.inventory.node(path), options.graph),
  );
  const dialog = new CanvasResolutionDialog(options.document);
  return {
    provisioning,
    workflow: () => state.loaded(),
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
      subscribeFocus: (listener) => route.subscribeFocus(listener),
    },
    cards: new ComfyInputSurfaceCards(
      associations,
      route,
      appearance,
      provisioning,
      regionalPrompts,
      regionalHover,
      new WorkflowMaskImport(provisioning, new BrowserMaskImport(options.document), options.assets),
      options.bindings,
    ),
    regions: new ComfyRegionalPromptLinks(associations, regionalHover, accent),
    resolutionEntries: (targets) => canvasResolutionEntries(targets, resolution, dialog, report),
    withNodeMenuItems: (node, items) => [
      ...items,
      ...canvasResolutionNodeMenuItems(
        isRecord(node) ? associations.inventory.path(node) : null,
        resolution,
        dialog,
        report,
      ),
    ],
    dispose: () => {
      dialog.close();
      stopAccent();
      promptNames.dispose();
      regionalHover.dispose();
      regionalPrompts.dispose();
      route.dispose();
      provisioning.dispose();
      graph.dispose();
    },
  };
}
