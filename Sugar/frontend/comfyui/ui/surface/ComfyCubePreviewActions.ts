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
/** Adapt Comfy's native image actions to Cube output media. */

import type { CubePreviewActions } from './CubePreviewActions.js';
import type { CubePreviewItem } from './CubePreviewModel.js';
import type { UnknownRecord } from '../types/common.js';
import { ContextMenu } from '../menus/ContextMenu.js';
import type { CanvasRevealTarget } from '../output_canvas/domain/CanvasReveal.js';

export interface ComfyPreviewMenuOption {
  content: string;
  callback(): void | Promise<void>;
}

export interface ComfyCubePreviewActionsOptions {
  document: Document;
  openWindow(url: string): void;
  writeClipboard(blob: Blob, image: HTMLImageElement): Promise<void>;
  fetchBlob(url: string): Promise<Blob>;
  showContextMenu(options: readonly ComfyPreviewMenuOption[], event: MouseEvent): void;
  reportError(message: string, error: unknown): void;
  revealImage?(target: CanvasRevealTarget): Promise<boolean>;
}

export interface ComfyCubePreviewActionHost {
  document: Document;
  liteGraph: UnknownRecord;
  logger: Pick<Console, 'error' | 'warn'>;
  revealImage?(target: CanvasRevealTarget): Promise<boolean>;
}

/** Reuse Comfy's image-action semantics on output media outside native node cards. */
export class ComfyCubePreviewActions implements CubePreviewActions {
  readonly #document: Document;
  readonly #openWindow: (url: string) => void;
  readonly #writeClipboard: (blob: Blob, image: HTMLImageElement) => Promise<void>;
  readonly #fetchBlob: (url: string) => Promise<Blob>;
  readonly #showContextMenu: (
    options: readonly ComfyPreviewMenuOption[],
    event: MouseEvent,
  ) => void;
  readonly #reportError: (message: string, error: unknown) => void;
  readonly #revealImage: ComfyCubePreviewActionsOptions['revealImage'];

  /** Bind browser operations without coupling preview views to host globals. */
  constructor(options: ComfyCubePreviewActionsOptions) {
    this.#document = options.document;
    this.#openWindow = options.openWindow;
    this.#writeClipboard = options.writeClipboard;
    this.#fetchBlob = options.fetchBlob;
    this.#showContextMenu = options.showContextMenu;
    this.#reportError = options.reportError;
    this.#revealImage = options.revealImage;
  }

  /** Open the same image commands exposed by Comfy's native node menu. */
  openContextMenu(item: CubePreviewItem, event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();
    const sourceUrl = resolveOriginalImageUrl(item.url, this.#document.baseURI);
    this.#showContextMenu(
      [
        ...(item.revealTarget && this.#revealImage
          ? [{ content: 'Show in output canvas', callback: () => this.reveal(item) }]
          : []),
        {
          content: 'Open Image',
          callback: () => this.#openWindow(sourceUrl),
        },
        {
          content: 'Copy Image',
          callback: () => this.#copyImage(sourceUrl),
        },
        {
          content: 'Save Image',
          callback: () => this.#saveImage(sourceUrl),
        },
      ],
      event,
    );
  }

  /** Match Comfy's hover download action without retaining preview transforms. */
  download(item: CubePreviewItem): void {
    this.#saveImage(resolveOriginalImageUrl(item.url, this.#document.baseURI));
  }

  /** Inspect only a completed thumbnail's captured identity through the canvas authority. */
  reveal(item: CubePreviewItem): void {
    if (!item.revealTarget || !this.#revealImage) return;
    void this.#revealImage(item.revealTarget).catch((error: unknown) =>
      this.#reportError('SugarCubes could not reveal the output image.', error),
    );
  }

  /** Copy one host image while preserving Comfy's asynchronous failure boundary. */
  async #copyImage(url: string): Promise<void> {
    try {
      const blob = await this.#fetchBlob(url);
      const image = this.#document.createElement('img');
      image.src = url;
      await this.#writeClipboard(blob, image);
    } catch (error: unknown) {
      this.#reportError('SugarCubes could not copy a Cube output image.', error);
    }
  }

  /** Trigger the host browser's recoverable file download flow. */
  #saveImage(url: string): void {
    const anchor = this.#document.createElement('a');
    anchor.href = url;
    anchor.download = readFilename(url);
    this.#document.body.append(anchor);
    anchor.click();
    this.#document.defaultView?.requestAnimationFrame(() => anchor.remove());
  }
}

/** Construct the browser adapter from Comfy's current LiteGraph host. */
export function createComfyCubePreviewActions(
  host: ComfyCubePreviewActionHost,
): ComfyCubePreviewActions {
  const windowRef = host.document.defaultView;
  if (!windowRef) throw new Error('Cube preview actions require a browser window.');
  return new ComfyCubePreviewActions({
    document: host.document,
    ...(host.revealImage ? { revealImage: host.revealImage } : {}),
    openWindow: (url) => {
      windowRef.open(url, '_blank');
    },
    fetchBlob: async (url) => {
      const response = await windowRef.fetch(url);
      if (!response.ok)
        throw new Error(`Image request failed with status ${String(response.status)}.`);
      return response.blob();
    },
    writeClipboard: async (blob) => {
      if (!windowRef.ClipboardItem || !windowRef.navigator.clipboard) {
        throw new Error('Image clipboard writing is unavailable in this browser.');
      }
      await windowRef.navigator.clipboard.write([
        new windowRef.ClipboardItem({ [blob.type]: blob }),
      ]);
    },
    showContextMenu: (options, event) => {
      new ContextMenu(host.document).open({
        label: 'Image actions',
        anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
        entries: options.map((option) => ({
          label: option.content,
          select: () => {
            void option.callback();
          },
        })),
      });
    },
    reportError: (message, error) => host.logger.error(message, error),
  });
}

/** Remove Comfy's display-only preview transform from an image action URL. */
export function resolveOriginalImageUrl(value: string, baseUrl: string): string {
  const url = new URL(value, baseUrl);
  url.searchParams.delete('preview');
  return url.href;
}

/** Resolve the server-provided output filename used by native downloads. */
function readFilename(url: string): string {
  return new URL(url).searchParams.get('filename') ?? 'image';
}
