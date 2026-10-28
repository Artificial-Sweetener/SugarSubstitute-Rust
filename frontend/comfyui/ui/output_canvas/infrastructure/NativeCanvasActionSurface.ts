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

/** Advertise camera and admission operations only when the host can perform them. */
export class NativeCanvasActionSurface implements CanvasActionSurface {
  /** Keep transfers with the same qualified owner as the browser surface. */
  constructor(
    private readonly actions: CanvasMediaActions,
    private readonly openItem: (item: WorkflowOutputItem) => void,
  ) {}

  /** Camera commands are unavailable until the native host exposes them. */
  fit(): boolean {
    return false;
  }
  /** Native scale is unavailable until the native host exposes it. */
  native(): boolean {
    return false;
  }
  /** Zoom is unavailable until the native host exposes it. */
  zoom(): boolean {
    return false;
  }
  /** Pan is unavailable until the native host exposes it. */
  pan(): boolean {
    return false;
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
