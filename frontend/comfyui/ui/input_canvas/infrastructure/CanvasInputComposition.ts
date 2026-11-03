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
/** Compose reusable source capture with image and mask interpretation, persistence and masonry presentation. */
import type { CanvasMaskHost, CanvasSourceHost } from '../../core/embedded/CanvasRuntime.js';
import { CanvasSourceCapture } from '../application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../application/CanvasSourcePreviews.js';
import { InputSourcePreparation } from '../application/InputSourcePreparation.js';
import { BrowserCanvasSourceAssets } from './BrowserCanvasSourceAssets.js';
import { LegacyMaskMigration } from './LegacyMaskMigration.js';
import { ComfyInputSourceDefaults } from './ComfyInputSourceDefaults.js';
import { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { ComfyInputPreparation } from './ComfyInputPreparation.js';
import { MasonryInputHost } from './MasonryInputHost.js';
import { ComfyInputBindings } from './ComfyInputBindings.js';
import { BrowserInputPreviews } from './BrowserInputPreviews.js';
import { BrowserInputRasters } from './BrowserInputRasters.js';
import { BrowserMaskEditing } from './BrowserMaskEditing.js';
import { MaskInputPreparation } from '../application/MaskInputPreparation.js';
import { ComfyMaskInputPreparation } from './ComfyMaskInputPreparation.js';
import { PreparedInputPromptClient } from './PreparedInputPromptClient.js';
import { ComfyInputReturnPreparation } from './ComfyInputReturnPreparation.js';
import { ComfyInputAssociations } from './ComfyInputAssociations.js';
import type { ComfyGraph } from '../../types/graph.js';
import type { MemoryBudgetRegistry } from '../../core/memory/MemoryBudgetRegistry.js';
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type { InputCanvasCapabilities } from '../application/InputEditorPorts.js';
import type { WorkflowSurfaceNotice } from '../application/WorkflowSurfaceProvisioning.js';
import { createWorkflowSurfaces, type WorkflowSurfaces } from './WorkflowSurfaceComposition.js';

/** Supply host capabilities and native root access at the composition boundary. */
export interface CanvasInputHostOptions {
  readonly document: Document;
  host(id?: string): CanvasSourceHost | null;
  readonly canvases: InputCanvasCapabilities;
  workflowHost(): CanvasWorkflowHost | null;
  /** Theme accent that colours the first mask of each input surface. */
  accent(): string;
  notice: WorkflowSurfaceNotice;
  legacyHost(id?: string): CanvasMaskHost | null;
  graph(): ComfyGraph | null | undefined;
  report(error: unknown): void;
  readonly logger: Pick<Console, 'warn'>;
  /** Page allowance shared by decoded source previews. */
  readonly memory: MemoryBudgetRegistry;
}

/** Keep definition registration, execution preparation and view lifetime under explicit owners. */
export function createCanvasInputs(options: CanvasInputHostOptions): {
  loaders: ComfyInputLoaders;
  preparation: ComfyInputPreparation;
  masonry: MasonryInputHost;
  bindings: ComfyInputBindings;
  associations: ComfyInputAssociations;
  surfaces: WorkflowSurfaces;
  define(definitions: unknown): void;
  setup(): void;
  beforeConfigureGraph(): void;
  afterConfigureGraph(): Promise<void>;
} {
  const { document, host, legacyHost, report } = options;
  const loaders = new ComfyInputLoaders();
  const assets = new BrowserCanvasSourceAssets((input, init) => fetch(input, init));
  const capture = new CanvasSourceCapture(host, (canvasId, documentId) =>
    surfaces.provisioning.document(canvasId, documentId),
  );
  const rasters = new BrowserInputRasters(document, (input, init) => fetch(input, init));
  const maskEditing = new BrowserMaskEditing();
  const previews = new CanvasSourcePreviews(capture, host, report);
  const legacy = new LegacyMaskMigration(legacyHost, assets);
  const defaults = new ComfyInputSourceDefaults(host, legacy, (id) =>
    options.canvases.provisions(id),
  );
  const bindings = new ComfyInputBindings(loaders, {
    host,
    canvases: options.canvases,
    refreshPreviews: (sources) => previews.refreshSources(sources),
    files: new BrowserInputPreviews((source) => assets.stage(source), rasters, options.memory),
    maskEditing,
    legacy,
    verifyFile: (file) => assets.verify(file),
    verifiedFile: (file) => assets.verified(file),
    report,
  });
  const associations = new ComfyInputAssociations(
    loaders,
    bindings,
    options.graph,
    report,
    options.logger,
  );
  const surfaces = createWorkflowSurfaces({
    document,
    associations,
    bindings,
    loaders,
    graph: options.graph,
    workflowHost: options.workflowHost,
    accent: options.accent,
    notice: options.notice,
    report,
  });
  const masonry = new MasonryInputHost(
    document,
    loaders,
    previews,
    (file) => assets.file(file),
    report,
    bindings,
    associations,
    surfaces.cards,
  );
  const canvasReady = (): void => {
    void surfaces.provisioning.refresh();
  };
  document.addEventListener('sugarcubes-canvas-ready', canvasReady);
  document.defaultView?.addEventListener(
    'pagehide',
    () => {
      document.removeEventListener('sugarcubes-canvas-ready', canvasReady);
      surfaces.dispose();
      associations.dispose();
      bindings.clear();
      maskEditing.dispose();
    },
    { once: true },
  );
  return {
    loaders,
    preparation: new ComfyInputPreparation(
      loaders,
      new InputSourcePreparation(capture, assets),
      defaults,
      new ComfyInputReturnPreparation(rasters, async () => {
        const source = host();
        if (!source) return null;
        const context = await capture.context(source.canvasId);
        return context.document ? { canvasId: source.canvasId, document: context.document } : null;
      }),
      new ComfyMaskInputPreparation(rasters, assets, new MaskInputPreparation(maskEditing), () =>
        associations.preparationPairs(),
      ),
      new PreparedInputPromptClient((input, init) => fetch(input, init)),
    ),
    masonry,
    bindings,
    associations,
    surfaces,
    define: (definitions) => {
      loaders.define(definitions);
      associations.definitionsChanged();
    },
    setup: () => {
      associations.start();
      masonry.setup();
      void surfaces.provisioning.refresh();
    },
    beforeConfigureGraph: () => {
      masonry.beforeConfigureGraph();
      associations.beforeConfigureGraph();
      bindings.clear();
    },
    afterConfigureGraph: async () => {
      associations.start();
      void surfaces.provisioning.refresh();
      await bindings.ready();
    },
  };
}
