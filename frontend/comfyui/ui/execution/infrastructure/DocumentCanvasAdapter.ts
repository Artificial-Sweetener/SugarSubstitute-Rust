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
      layers: host,
      get geometry() {
        const capabilities = host.readCanvas()?.capabilities;
        return capabilities && 'geometry' in capabilities && capabilities.geometry
          ? (host.geometry ?? undefined)
          : undefined;
      },
      get sources() {
        return host.sources ?? undefined;
      },
      get masks() {
        return host.masks ?? undefined;
      },
      get surface() {
        return host.surface ?? undefined;
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
                  const returned = output.canvasReturns?.find(
                    (value) => value.executionPath === output.source.executionPath,
                  );
                  if (returned?.kind === 'unavailable') throw new Error(returned.message);
                  if (
                    returned?.kind === 'region' &&
                    (returned.canvasId !== context.canvas.id ||
                      !context.canvas.capabilities.positionImageLayer)
                  )
                    throw new Error('This canvas cannot receive the captured positioned inpaint.');
                  for (const { media, batchIndex } of output.images) {
                    if (stop.cancelled()) return;
                    try {
                      const region =
                        returned?.kind === 'region'
                          ? returned.regions.length === 1
                            ? returned.regions[0]
                            : returned.regions[batchIndex]
                          : null;
                      if (returned?.kind === 'region' && !region)
                        throw new Error('The inpaint batch has no matching captured mask.');
                      const prepared = await images.load(media.file, stop, region ?? null);
                      if (!prepared) continue;
                      if (stop.cancelled()) return;
                      const result = await host.insertImage(
                        {
                          document: returned?.kind === 'region' ? returned.document : document,
                          image: prepared.image,
                          position: prepared.position,
                          layerName:
                            media.file.filename.replace(/\.png$/i, '').slice(0, 128) ||
                            'Generated image',
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
