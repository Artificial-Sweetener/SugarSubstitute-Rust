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
/** Compose the embedded document adapter with the common canvas coordinator. */
import type { CanvasLayerHost } from '../../core/embedded/CanvasRuntime.js';
import type { WorkflowCanvasCoordinator } from '../application/WorkflowCanvasCoordinator.js';
import { documentCanvasAdapter } from './DocumentCanvasAdapter.js';
import { BrowserCanvasGenerationImages } from './BrowserCanvasGenerationImages.js';

/** Own the page registration and feedback without a separate execution-delivery subscription. */
export class ComfyDocumentCanvasHost {
  readonly #unregister: () => void;
  /** Translate document-host primitives into the same capability contract as other canvases. */
  constructor(
    canvases: WorkflowCanvasCoordinator<Blob>,
    private readonly canvas: CanvasLayerHost,
    private readonly window: Window,
    logger: Pick<Console, 'error'>,
    notify: (message: string) => void,
  ) {
    this.#unregister = canvases.register(
      documentCanvasAdapter(
        'embedded-document',
        canvas,
        new BrowserCanvasGenerationImages(window.location.origin, (input, init) =>
          window.fetch(input, init),
        ),
        (error) => {
          logger.error('SugarCubes document canvas operation failed.', { error });
          notify(error instanceof Error ? error.message : String(error));
        },
      ),
    );
    window.addEventListener('pagehide', this.dispose);
  }
  /** Select this destination only while the authenticated document host advertises insertion. */
  get available(): boolean {
    return this.canvas.readCanvas()?.capabilities.insertImageLayer ?? false;
  }
  /** Retire this adapter while leaving other canvases registered. */
  readonly dispose = (): void => {
    this.window.removeEventListener('pagehide', this.dispose);
    this.#unregister();
  };
}
