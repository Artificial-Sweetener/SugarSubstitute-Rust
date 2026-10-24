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
/** Translate shared canvas output delivery into the document host's image-layer primitive. */
import type {
  CanvasCancellationSource,
  CanvasLayerHost,
} from '../../core/embedded/CanvasRuntime.js';
import type { WorkflowCanvasAdapter, WorkflowCanvasOutput } from '../domain/WorkflowCanvas.js';
import type { CanvasGenerationImages } from '../domain/CanvasGenerationImages.js';

/** Adapt document operations without owning execution subscriptions, identity, queues or recovery. */
export function documentCanvasAdapter<Bytes>(
  id: string,
  host: CanvasLayerHost,
  images: CanvasGenerationImages,
  report: (error: unknown) => void,
): WorkflowCanvasAdapter<Bytes> {
  return {
    id,
    capabilities: {
      get sources() {
        return host.sources ?? undefined;
      },
      get masks() {
        return host.masks ?? undefined;
      },
      output: {
        ordering: 'serial',
        bind(_submission, cancellation) {
          if (!host.readCanvas()?.capabilities.insertImageLayer) return null;
          return host
            .readContext()
            .then((context) => {
              if (!context.canvas.capabilities.insertImageLayer)
                throw new Error('This canvas does not support image layers.');
              const document = context.document;
              if (!document)
                throw new Error(
                  `Open a document in ${context.canvas.label} before generating an image.`,
                );
              if (cancellation.cancelled()) return null;
              return {
                async receive(output: WorkflowCanvasOutput, stop: CanvasCancellationSource) {
                  for (const { media } of output.images) {
                    if (stop.cancelled()) return;
                    try {
                      const image = await images.load(media.file, stop);
                      if (stop.cancelled()) return;
                      const result = await host.insertImage(
                        {
                          document,
                          image,
                          layerName: media.file.filename.slice(0, 128) || 'Generated image',
                        },
                        stop,
                      );
                      if (result.status !== 'inserted' && result.status !== 'cancelled')
                        throw new Error(result.message);
                    } catch (error: unknown) {
                      if (!stop.cancelled()) report(error);
                    }
                  }
                },
              };
            })
            .catch((error: unknown) => {
              if (!cancellation.cancelled()) report(error);
              return null;
            });
        },
      },
    },
  };
}
