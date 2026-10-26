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
/** Compose native canvas ports while retaining replay and lifetime ownership in the shared server. */
import type { CanvasMessage } from '../../../contracts/CanvasProtocol.generated.js';
import type {
  CanvasMaskMessage,
  CanvasMaskChanged,
} from '../../../contracts/CanvasMasksProtocol.generated.js';
import { nativeCanvasChanges } from '../../../shared/CanvasChangeRelay.js';
import { CanvasMaskServer } from '../../../shared/CanvasMaskServer.js';
import { PhotoshopCanvasMasks } from './PhotoshopCanvasMasks.js';
import { PhotoshopCanvasSources } from './PhotoshopCanvasSources.js';
import { CanvasServer } from '../../../shared/CanvasServer.js';
import { InsertCanvasImage } from '../application/InsertCanvasImage.js';
import { PhotoshopImageAssets } from './PhotoshopImageAssets.js';
import { PhotoshopCanvasDocuments } from './PhotoshopCanvasDocuments.js';
import type { PhotoshopMaskSessionPort } from './PhotoshopMaskSession.js';
/** Scope each canvas connection to the same authenticated navigation as its panel. */
export class PhotoshopCanvasBridge {
  #server: CanvasServer | null = null;
  #maskServer: CanvasMaskServer | null = null;
  #masks: PhotoshopCanvasMasks | null = null;
  #sources: PhotoshopCanvasSources | null = null;
  readonly #documents: PhotoshopCanvasDocuments;
  readonly #insertion: InsertCanvasImage;
  /** Keep native services lazy so registration itself never opens documents or accesses storage. */
  constructor(
    private readonly send: (message: CanvasMessage | CanvasMaskMessage) => void,
    private readonly report: (error: unknown) => void,
    private readonly maskSession: PhotoshopMaskSessionPort,
  ) {
    const assets = new PhotoshopImageAssets();
    this.#documents = new PhotoshopCanvasDocuments(assets, report);
    this.#insertion = new InsertCanvasImage({
      prepare: (image, cancel) => assets.prepare(image, cancel),
      insert: (request, image, cancel) => this.#documents.insert(request, image, cancel),
      report,
    });
  }
  /** Cancel the previous navigation before accepting requests in the new session. */
  start(session: string): void {
    this.dispose();
    this.#sources = new PhotoshopCanvasSources(this.report);
    this.#server = new CanvasServer(session, {
      sources: this.#sources,
      context: () => Promise.resolve(this.#documents.context()),
      insert: (image, cancel) => this.#insertion.execute(image, cancel),
      send: this.send,
      report: this.report,
    });
    const maskChanges = new Set<(change: CanvasMaskChanged) => void>();
    this.#masks = new PhotoshopCanvasMasks(
      (documentId, maskId, revision) => {
        for (const listener of maskChanges)
          listener({
            protocol: 'sugarcubes.canvas.masks',
            version: 2,
            session,
            type: 'changed',
            documentId,
            maskId,
            revision,
          });
      },
      this.report,
      this.maskSession,
    );
    this.#maskServer = new CanvasMaskServer(session, {
      changes: nativeCanvasChanges((listener) => {
        maskChanges.add(listener);
        return () => {
          maskChanges.delete(listener);
        };
      }),
      execute: (command) =>
        this.#masks?.execute(command) ??
        Promise.resolve({
          kind: 'failure',
          status: 'unavailable',
          message: 'Photoshop masks are unavailable.',
        }),
      send: this.send,
      report: this.report,
    });
  }
  /** Consume only validated canvas envelopes from the panel's trusted origin/source boundary. */
  receive(value: unknown): void {
    this.#server?.receive(value);
    this.#maskServer?.receive(value);
  }
  /** Cancel in-flight image insertion and prevent stale navigation responses. */
  dispose(): void {
    this.#server?.dispose();
    this.#server = null;
    this.#sources?.dispose();
    this.#sources = null;
    this.#maskServer?.dispose();
    this.#maskServer = null;
    this.#masks?.dispose();
    this.#masks = null;
  }
}
