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
/** Route supported shared output actions without allocating browser image resources. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasActionSurface } from '../application/CanvasActions.js';
import type { CanvasCommandScope } from '../domain/CanvasViewPort.js';
import type { CanvasMediaActions } from './CanvasMediaActions.js';
import type { CanvasSurfaceViewportCommand } from '../../../../../integrations/contracts/CanvasSurface.js';

/** Advertise camera and admission operations only when the host can perform them. */
export class NativeCanvasActionSurface implements CanvasActionSurface {
  /** Keep transfers with the same qualified owner as the browser surface. */
  constructor(
    private readonly actions: CanvasMediaActions,
    private readonly openItem: (item: WorkflowOutputItem) => void,
    private readonly viewport: (
      command: CanvasSurfaceViewportCommand,
      scope: CanvasCommandScope,
    ) => Promise<boolean>,
  ) {}

  /** Fit through the native presentation's existing revision stream. */
  fit(scope: CanvasCommandScope): Promise<boolean> {
    return this.viewport({ type: 'fit' }, scope);
  }
  /** Request one source pixel per physical display pixel from the native camera. */
  native(scope: CanvasCommandScope): Promise<boolean> {
    return this.viewport({ type: 'native' }, scope);
  }
  /** Keep factor interpretation and clamping with the native camera. */
  zoom(factor: number, scope: CanvasCommandScope): Promise<boolean> {
    return this.viewport({ type: 'zoom', factor }, scope);
  }
  /** Forward viewport fractions without duplicating display-density conversion. */
  pan(x: number, y: number, scope: CanvasCommandScope): Promise<boolean> {
    return this.viewport({ type: 'pan', x, y, units: 'viewport' }, scope);
  }

  /** Native image retries require a host-owned retry operation. */
  retry(): boolean {
    return false;
  }
  /** Native image admission remains with the native resource owner. */
  admitLargeImage(): boolean {
    return false;
  }
  /** Open the exact selected output through the host's existing action. */
  open(item: WorkflowOutputItem): void {
    this.openItem(item);
  }
  /** Execute a qualified user-requested transfer. */
  media(
    command: Parameters<CanvasActionSurface['media']>[0],
    scope: CanvasCommandScope,
  ): Promise<boolean> {
    return this.actions.run(command, scope);
  }
}
