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
/** Resolve Comfy-owned decoded preview images across renderer modes. */

import { isRecord } from '../types/common.js';

interface LocatedImage {
  image: HTMLImageElement;
  locator: string | null;
}

/** Find decoded output media without taking ownership away from Comfy nodes. */
export class ComfyGraphPreviewImageSource {
  readonly #document: Document;
  readonly #rootGraph: object;
  readonly #logger: Pick<Console, 'debug'> | null;
  #lastMissingSignature = '';

  /** Bind the current document and authoritative root graph. */
  constructor(
    documentRef: Document,
    rootGraph: object,
    logger: Pick<Console, 'debug'> | null = null,
  ) {
    this.#document = documentRef;
    this.#rootGraph = rootGraph;
    this.#logger = logger;
  }

  /** Return Comfy's decoded image for one stable output URL. */
  find(url: string, sourceLocator?: string): HTMLImageElement | null {
    const requested = parseUrl(url, this.#document.baseURI);
    if (!requested) return null;
    const images = this.#images().filter(
      ({ image }) => image.naturalWidth > 0 && image.naturalHeight > 0,
    );
    for (const { image } of images) {
      const candidate = parseUrl(image.currentSrc || image.src, this.#document.baseURI);
      if (!candidate) continue;
      if (candidate.href === requested.href || isSameComfyView(candidate, requested)) return image;
    }
    if (sourceLocator) {
      const located = images.find(({ locator }) => locator === sourceLocator)?.image ?? null;
      if (located) return located;
      const signature = JSON.stringify({
        sourceLocator,
        availableLocators: [...new Set(images.map(({ locator }) => locator).filter(Boolean))],
      });
      if (signature !== this.#lastMissingSignature) {
        this.#lastMissingSignature = signature;
        this.#logger?.debug(`SugarCubes could not reuse a decoded Comfy preview. ${signature}`);
      }
    }
    return null;
  }

  /** Collect DOM and graph-node images while traversing nested subgraphs once. */
  #images(): LocatedImage[] {
    const images: LocatedImage[] = Array.from(this.#document.images).map((image) => ({
      image,
      locator: null,
    }));
    const imageConstructor = this.#document.defaultView?.HTMLImageElement;
    if (!imageConstructor) return images;
    const pending: Array<{ graph: object; root: boolean }> = [
      { graph: this.#rootGraph, root: true },
    ];
    const visited = new Set<object>();
    while (pending.length > 0) {
      const next = pending.pop();
      if (!next || visited.has(next.graph)) continue;
      const { graph, root } = next;
      visited.add(graph);
      const record = isRecord(graph) ? graph : null;
      const nodes = Array.isArray(record?._nodes)
        ? record._nodes
        : Array.isArray(record?.nodes)
          ? record.nodes
          : [];
      for (const value of nodes) {
        if (!isRecord(value)) continue;
        const nodeId = readNonEmptyString(value.id);
        const graphId = readNonEmptyString(record?.id);
        const locator = !root && graphId && nodeId ? `${graphId}:${nodeId}` : nodeId;
        if (Array.isArray(value.imgs)) {
          for (const image of value.imgs) {
            if (image instanceof imageConstructor) images.push({ image, locator });
          }
        }
        if (isRecord(value.subgraph)) pending.push({ graph: value.subgraph, root: false });
      }
    }
    return images;
  }
}

/** Read one stable external graph identity. */
function readNonEmptyString(value: unknown): string | null {
  if (typeof value === 'string') return value.trim() || null;
  return typeof value === 'number' && Number.isFinite(value) ? String(value) : null;
}

/** Parse one host-provided image URL without leaking invalid values outward. */
function parseUrl(value: string, baseUrl: string): URL | null {
  try {
    return new URL(value, baseUrl);
  } catch {
    return null;
  }
}

/** Compare stable Comfy output identity while ignoring cache-busting parameters. */
function isSameComfyView(candidate: URL, requested: URL): boolean {
  if (candidate.origin !== requested.origin || candidate.pathname !== requested.pathname)
    return false;
  return ['filename', 'subfolder', 'type'].every(
    (key) => candidate.searchParams.get(key) === requested.searchParams.get(key),
  );
}
