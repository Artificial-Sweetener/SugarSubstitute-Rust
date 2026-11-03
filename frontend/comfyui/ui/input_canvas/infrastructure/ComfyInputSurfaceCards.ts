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
import type { WorkflowSurfaceProvisioning } from '../application/WorkflowSurfaceProvisioning.js';
import type { ComfyInputAssociations } from './ComfyInputAssociations.js';

/** Resolve the card's endpoint at use time so renumbered or moved nodes stay addressed. */
export class ComfyInputSurfaceCards {
  /** Share the route, appearance and provisioning owners with the canvas tab controls. */
  constructor(
    private readonly associations: ComfyInputAssociations,
    private readonly route: Pick<WorkflowInputRoute, 'activateImage' | 'activateMask'>,
    private readonly appearance: Pick<
      WorkflowMaskAppearance,
      'hover' | 'opacity' | 'changeOpacity'
    >,
    private readonly provisioning: Pick<WorkflowSurfaceProvisioning, 'subscribe'>,
  ) {}

  /** Bind one loader field's card. */
  port(node: ComfyNode, loader: InputLoader): InputSurfaceCardPort {
    const endpoint = (): string | null =>
      this.associations.inventory.workflowKey(this.associations.key(node, loader.field));
    return {
      activate: (entry) => {
        const key = endpoint();
        if (!key) return;
        if (loader.kind === 'image') this.route.activateImage(key);
        else this.route.activateMask(key, loader.ordered ? entry : null);
      },
      hover: (entry) => {
        const key = endpoint();
        if (key && loader.kind === 'mask' && loader.ordered) this.appearance.hover(key, entry);
      },
      opacity: () => {
        const key = endpoint();
        return key && loader.kind === 'mask' ? this.appearance.opacity(key) : null;
      },
      changeOpacity: (opacity) => {
        const key = endpoint();
        if (key && loader.kind === 'mask') this.appearance.changeOpacity(key, opacity);
      },
      subscribe: (listener) => this.provisioning.subscribe(listener),
    };
  }
}
