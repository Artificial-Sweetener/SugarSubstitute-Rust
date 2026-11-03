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
/** Address a live loader card's input surfaces by the loader's stable workflow key. */
import type { ComfyNode } from '../../types/graph.js';
import type { InputLoader } from '../domain/InputLoader.js';
import type { InputSurfaceCardPort } from '../application/InputEditorPorts.js';
import type { WorkflowInputRoute } from '../application/WorkflowInputRoute.js';
import type { WorkflowMaskAppearance } from '../application/WorkflowMaskAppearance.js';
import type { WorkflowRegionalHover } from '../application/WorkflowRegionalHover.js';
import type { WorkflowRegionalPrompts } from '../application/WorkflowRegionalPrompts.js';
import type { WorkflowSurfaceProvisioning } from '../application/WorkflowSurfaceProvisioning.js';
import type { ComfyInputAssociations } from './ComfyInputAssociations.js';
import type { ComfyInputBindings } from './ComfyInputBindings.js';
import type { WorkflowMaskImport } from '../application/WorkflowMaskImport.js';

/** Resolve the card's endpoint at use time so renumbered or moved nodes stay addressed. */
export class ComfyInputSurfaceCards {
  /** Share the route, appearance, provisioning and regional owners with the canvas tab controls. */
  constructor(
    private readonly associations: ComfyInputAssociations,
    private readonly route: Pick<WorkflowInputRoute, 'activateImage' | 'activateMask'>,
    private readonly appearance: Pick<WorkflowMaskAppearance, 'opacity' | 'changeOpacity'>,
    private readonly provisioning: Pick<WorkflowSurfaceProvisioning, 'subscribe'>,
    private readonly regions: Pick<WorkflowRegionalPrompts, 'region' | 'subscribe'>,
    private readonly regionalHover: Pick<
      WorkflowRegionalHover,
      'hoverMask' | 'hovered' | 'subscribe'
    >,
    private readonly imports: Pick<WorkflowMaskImport, 'available' | 'import'>,
    private readonly bindings: Pick<ComfyInputBindings, 'get'>,
  ) {}

  /** Bind one loader field's card. */
  port(node: ComfyNode, loader: InputLoader): InputSurfaceCardPort {
    const endpoint = (): string | null =>
      this.associations.inventory.workflowKey(this.associations.key(node, loader.field));
    // A mixed image loader whose only used output is MASK acts as a mask endpoint.
    const kind = () =>
      this.associations.inventory.kind(this.associations.key(node, loader.field)) ?? loader.kind;
    const regional = (): string | null => {
      const key = endpoint();
      return key && kind() === 'mask' && loader.ordered ? key : null;
    };
    return {
      activate: (entry) => {
        const key = endpoint();
        if (!key) return;
        if (kind() === 'image') this.route.activateImage(key);
        else this.route.activateMask(key, loader.ordered ? entry : null);
      },
      hover: (entry) => {
        const key = regional();
        if (key) this.regionalHover.hoverMask(key, entry);
      },
      hovered: () => {
        const key = regional();
        return key ? this.regionalHover.hovered(key) : null;
      },
      subscribeHover: (listener) => this.regionalHover.subscribe(listener),
      region: (entry) => {
        const key = regional();
        return key ? this.regions.region(key, entry) : null;
      },
      opacity: () => {
        const key = endpoint();
        return key && kind() === 'mask' ? this.appearance.opacity(key) : null;
      },
      changeOpacity: (opacity) => {
        const key = endpoint();
        if (key && kind() === 'mask') this.appearance.changeOpacity(key, opacity);
      },
      canImport: () => {
        const key = endpoint();
        return Boolean(key && kind() === 'mask' && loader.ordered && this.imports.available(key));
      },
      importMasks: async (files) => {
        const key = endpoint();
        if (!key || kind() !== 'mask' || !loader.ordered)
          throw new Error('This loader cannot import masks.');
        await this.imports.import(key, files, (staged) =>
          this.bindings.get(node, loader).append(staged),
        );
      },
      subscribe: (listener) => {
        const stopProvisioning = this.provisioning.subscribe(listener),
          stopRegions = this.regions.subscribe(listener);
        return () => {
          stopProvisioning();
          stopRegions();
        };
      },
    };
  }
}
