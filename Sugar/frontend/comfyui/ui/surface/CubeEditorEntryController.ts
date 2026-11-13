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
/** Authorize and coordinate entry from a Cube surface into its definition editor. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';

interface CubeEditorEntryControllerOptions {
  canEdit(node: CubeNode): Promise<boolean>;
  leaveSurface(): void;
  recoverSurface(): void;
  openEditor(node: CubeNode): void;
  onDenied(node: CubeNode): void;
  logger: Pick<Console, 'error'>;
}

/** Own the permission-checked transition from presentation into Cube editing. */
export class CubeEditorEntryController {
  readonly #options: CubeEditorEntryControllerOptions;

  /** Bind the editor policy and transition collaborators. */
  constructor(options: CubeEditorEntryControllerOptions) {
    this.#options = options;
  }

  /** Request editor entry without allowing host-native navigation to bypass policy. */
  open(node: CubeNode): void {
    void this.#openAuthorized(node);
  }

  /** Resolve access before changing any mounted surface state. */
  async #openAuthorized(node: CubeNode): Promise<void> {
    try {
      if (!(await this.#options.canEdit(node))) {
        this.#options.onDenied(node);
        return;
      }
      this.#options.leaveSurface();
      this.#options.openEditor(node);
    } catch (error: unknown) {
      this.#options.recoverSurface();
      this.#options.logger.error('SugarCubes could not open the Cube editor.', {
        cubeNodeId: node.id,
        error,
      });
    }
  }
}
