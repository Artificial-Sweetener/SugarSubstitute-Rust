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
/** Adapt completed Comfy outputs to the shared model-thumbnail choice contract. */
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ModelOutputThumbnailChoice, ModelOutputThumbnails } from './ModelOutputThumbnails.js';
import type { WorkflowOutputSet } from '../execution/domain/WorkflowOutput.js';
import { readComfyOutputImage } from './ComfyOutputImageDownload.js';
import { buildCubeOutputExecutionId } from '../cube/execution/CubeOutputExecutionIdentity.js';

interface OutputOrigin {
  readonly node: UnknownRecord;
  readonly index: number;
}

interface LocatedOutput {
  readonly choice: ModelOutputThumbnailChoice;
  readonly url: string;
  readonly filename: string;
  readonly origins: OutputOrigin[];
}

/** Ignore transient previews, orphaned outputs and arbitrary external image URLs. */
export class ComfyOutputThumbnailSource implements ModelOutputThumbnails {
  readonly #app: unknown;
  readonly #document: Document;
  readonly #output: (executionId: string) => WorkflowOutputSet | null;

  /** Read the live host only when a menu opens or an image is explicitly chosen. */
  constructor(
    app: unknown,
    documentRef: Document,
    output: (executionId: string) => WorkflowOutputSet | null,
  ) {
    this.#app = app;
    this.#document = documentRef;
    this.#output = output;
  }

  /** Return each concrete completed image once in graph and batch order. */
  choices(): readonly ModelOutputThumbnailChoice[] {
    return this.#outputs().map((output) => output.choice);
  }

  /** Offer Current image only when Comfy has one selected node and a concrete image index. */
  activeChoice(): ModelOutputThumbnailChoice | null {
    const app = isRecord(this.#app) ? this.#app : {};
    const canvas = isRecord(app.canvas) ? app.canvas : {};
    const selected = isRecord(canvas.selected_nodes) ? Object.values(canvas.selected_nodes) : [];
    if (selected.length !== 1) return null;
    const node = selected[0];
    return isRecord(node) && Number.isInteger(node.imageIndex)
      ? (this.#outputs().find((output) =>
          output.origins.some((origin) => origin.node === node && origin.index === node.imageIndex),
        )?.choice ?? null)
      : null;
  }

  /** Revalidate the chosen identity against the current workflow before reading any bytes. */
  async image(id: string, signal: AbortSignal): Promise<File> {
    const output = this.#outputs().find((candidate) => candidate.choice.id === id);
    if (!output) throw new Error('That output image is no longer available.');
    return readComfyOutputImage(output.url, output.filename, signal);
  }

  /** Traverse native execution paths without interpreting cube internals as model policy. */
  #outputs(): readonly LocatedOutput[] {
    const app = isRecord(this.#app) ? this.#app : {};
    const result: LocatedOutput[] = [];
    const seenImages = new Map<string, LocatedOutput>();
    const pending: Array<{
      graph: unknown;
      path: readonly string[];
      title: string;
      ancestors: ReadonlySet<object>;
    }> = [{ graph: app.rootGraph ?? app.graph, path: [], title: '', ancestors: new Set() }];
    while (pending.length) {
      const next = pending.shift();
      if (!next || !isRecord(next.graph) || next.ancestors.has(next.graph) || next.path.length > 32)
        continue;
      const ancestors = new Set([...next.ancestors, next.graph]);
      const nodes = Array.isArray(next.graph._nodes) ? next.graph._nodes : next.graph.nodes;
      if (!Array.isArray(nodes)) continue;
      for (const node of nodes) {
        if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number'))
          continue;
        const path = [...next.path, String(node.id)];
        const locator = path.join(':');
        const title =
          typeof node.title === 'string' && node.title.trim()
            ? node.title
            : typeof node.type === 'string'
              ? node.type
              : 'Output';
        const sourceLabel = next.title ? `${next.title} / ${title}` : title;
        if (isRecord(node.subgraph))
          pending.push({ graph: node.subgraph, path, title: sourceLabel, ancestors });
        const boundaryOutputs =
          isRecord(node.subgraph) && Array.isArray(node.subgraph.outputs)
            ? node.subgraph.outputs
            : [];
        const candidates = boundaryOutputs.map((boundary: unknown, slot: number) => ({
          locator: buildCubeOutputExecutionId(locator, slot),
          sourceLabel:
            isRecord(boundary) && typeof boundary.name === 'string'
              ? `${sourceLabel} / ${boundary.name}`
              : sourceLabel,
          output: this.#output(buildCubeOutputExecutionId(locator, slot)),
        }));
        candidates.push({ locator, sourceLabel, output: this.#output(locator) });
        for (const candidate of candidates) {
          const output = candidate.output;
          if (!output) continue;
          output.items.forEach((item) => {
            const image = item.file;
            const index = item.batchIndex;
            if (item.mediaKind !== 'image' || (image.type !== 'output' && image.type !== 'temp'))
              return;
            const url = new URL('/view', this.#document.baseURI);
            url.searchParams.set('filename', image.filename);
            url.searchParams.set('type', image.type);
            url.searchParams.set(
              'subfolder',
              typeof image.subfolder === 'string' ? image.subfolder : '',
            );
            const origin = { node, index };
            const existing = seenImages.get(url.href);
            if (existing) {
              existing.origins.push(origin);
              return;
            }
            const located: LocatedOutput = {
              choice: {
                id: item.id,
                sceneKey: '',
                sceneTitle: '',
                sceneOrder: 0,
                sourceKey: candidate.locator,
                sourceLabel: candidate.sourceLabel,
                batch: index + 1,
              },
              url: url.href,
              filename: image.filename,
              origins: [origin],
            };
            seenImages.set(url.href, located);
            result.push(located);
          });
        }
      }
    }
    return result;
  }
}
