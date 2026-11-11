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
/** Compose native GIMP capabilities behind the existing authenticated shared canvas server. */
import type { CanvasMessage } from '../../../contracts/CanvasProtocol.generated.js';
import { CanvasServer } from '../../../shared/CanvasServer.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import { GimpCanvasDocuments } from './GimpCanvasDocuments.js';
import { GimpCanvasSources } from './GimpCanvasSources.js';
import { GimpCanvasGeometry } from './GimpCanvasGeometry.js';

/** Keep request correlation and replay ownership in CanvasServer. */
export class GimpCanvasBridge {
  readonly #server: CanvasServer;
  readonly #sources: GimpCanvasSources;

  /** Bind one public session to native document primitives without adding GIMP product commands. */
  constructor(
    session: string,
    native: GimpNativePort,
    send: (message: CanvasMessage) => void,
    report: (error: unknown) => void,
  ) {
    const documents = new GimpCanvasDocuments(native);
    this.#sources = new GimpCanvasSources(native, report);
    this.#server = new CanvasServer(session, {
      geometry: new GimpCanvasGeometry(native),
      sources: this.#sources,
      context: () => documents.context(),
      insert: (image, cancellation) => documents.insert(image, cancellation),
      send,
      report,
    });
  }

  /** Delegate authenticated public envelopes to the shared authoritative dispatcher. */
  receive(value: unknown): boolean {
    return this.#server.receive(value);
  }

  /** Retire the public session while leaving the shared native viewer lifetime with its owner. */
  dispose(): void {
    this.#server.dispose();
    this.#sources.dispose();
  }
}
