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
/** Compose reusable source capture with image and mask interpretation, persistence and masonry presentation. */
import type { CanvasMaskHost, CanvasSourceHost } from '../../core/embedded/CanvasRuntime.js';
import { CanvasSourceCapture } from '../application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../application/CanvasSourcePreviews.js';
import { InputSourcePreparation } from '../application/InputSourcePreparation.js';
import { BrowserCanvasSourceAssets } from './BrowserCanvasSourceAssets.js';
import { LegacyMaskMigration } from './LegacyMaskMigration.js';
import { ComfyInputSourceDefaults } from './ComfyInputSourceDefaults.js';
import { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { ComfyInputPreparation } from './ComfyInputPreparation.js';
import { MasonryInputHost } from './MasonryInputHost.js';
import { ComfyInputBindings } from './ComfyInputBindings.js';
import { BrowserInputPreviews } from './BrowserInputPreviews.js';

/** Keep definition registration, execution preparation and view lifetime under explicit owners. */
export function createCanvasInputs(
  document: Document,
  host: (id?: string) => CanvasSourceHost | null,
  legacyHost: (id?: string) => CanvasMaskHost | null,
  report: (error: unknown) => void,
): {
  loaders: ComfyInputLoaders;
  preparation: ComfyInputPreparation;
  masonry: MasonryInputHost;
  bindings: ComfyInputBindings;
  beforeConfigureGraph(): void;
} {
  const loaders = new ComfyInputLoaders();
  const assets = new BrowserCanvasSourceAssets((input, init) => fetch(input, init));
  const capture = new CanvasSourceCapture(host);
  const previews = new CanvasSourcePreviews(capture, host, report);
  const legacy = new LegacyMaskMigration(legacyHost, assets);
  const defaults = new ComfyInputSourceDefaults(host, legacy);
  const bindings = new ComfyInputBindings(loaders, {
    host,
    files: new BrowserInputPreviews(document, (source) => assets.stage(source)),
    legacy,
    verifyFile: (file) => assets.verify(file),
    verifiedFile: (file) => assets.verified(file),
    report,
  });
  const masonry = new MasonryInputHost(
    document,
    loaders,
    previews,
    (file) => assets.file(file),
    report,
    bindings,
  );
  document.defaultView?.addEventListener('pagehide', () => bindings.clear(), { once: true });
  return {
    loaders,
    preparation: new ComfyInputPreparation(
      loaders,
      new InputSourcePreparation(capture, assets),
      defaults,
    ),
    masonry,
    bindings,
    beforeConfigureGraph: () => {
      masonry.beforeConfigureGraph();
      bindings.clear();
    },
  };
}
