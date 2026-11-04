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
/** Render the actual shared input editor against a capture-only native test boundary. */
import { editMaskInput } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourcePreviews.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';
import { createComfyInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputBinding.js';
import { MasonryInputEditor } from '../../../frontend/comfyui/ui/input_canvas/presentation/MasonryInputEditor.js';
import { gimpTokenDeclarations } from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeTokens.js';
import { gimpAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/GimpAppearanceStyles.js';
import { encodeCanvasRaster } from '../../../frontend/comfyui/ui/core/embedded/CanvasRuntime.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { nativeInputFields, nativeSourceField } from './native_input_fields.js';

declare global {
  interface Window {
    captureOnlyProof: {
      edit(): void;
      read(): { captures: number; authored: string; errors: readonly string[] };
      dispose(): void;
    };
  }
}

/** Replace native pixels and Comfy's widget boundary while retaining capture, state, previews and controls. */
async function mount(): Promise<void> {
  document.documentElement.dataset.scVisualStyle = 'gimp';
  document.body.style.cssText = gimpTokenDeclarations(true);
  const appearance = document.createElement('style');
  appearance.textContent = gimpAppearanceCss(true);
  document.head.append(appearance);
  const nativeDocument = {
    id: 'test-document',
    geometryRevision: '160x90',
    width: 160,
    height: 90,
  };
  let revision = 1,
    captures = 0;
  const errors: string[] = [];
  const host: CanvasSourceHost = {
    canvasId: 'gimp-test-adapter',
    changes: null,
    execute: async (query) => {
      if (query.type === 'context')
        return {
          kind: 'context',
          supported: true,
          observesChanges: false,
          document: nativeDocument,
          revision: String(revision),
          targets: ['composition'],
          layers: [],
          currentLayerId: null,
        };
      captures++;
      const bytes = new Uint8Array(160 * 90 * 4);
      for (let index = 0; index < bytes.length; index += 4) {
        bytes[index] = revision === 1 ? 180 : 40;
        bytes[index + 1] = 100;
        bytes[index + 2] = revision === 1 ? 40 : 180;
        bytes[index + 3] = 255;
      }
      const pixels = encodeCanvasRaster(160, 90, 'rgba8', bytes);
      return {
        kind: 'captured',
        snapshots: query.targets.map((target) => ({
          target,
          document: nativeDocument,
          revision: String(revision),
          pixels,
        })),
      };
    },
  };
  const report = (error: unknown): void => {
    errors.push(error instanceof Error ? error.message : String(error));
    console.error('Capture-only browser proof failed', { error });
  };
  const loaders = new ComfyInputLoaders();
  loaders.define({ LoadImage: {} });
  const loader = loaders.read('LoadImage');
  if (!loader) throw new Error('Missing native image loader definition.');
  const node: ComfyNode = {
    id: 1,
    type: 'LoadImage',
    properties: {},
    widgets: [
      { name: 'image', value: '' },
      { name: 'upload', type: 'button' },
    ],
  };
  const previews = new CanvasSourcePreviews(
    new CanvasSourceCapture(() => host),
    () => host,
    report,
    () => null,
  );
  const binding = createComfyInputBinding(node, loader, {
    host: () => host,
    canvases: { editable: () => true, provisions: () => false },
    maskEditing: { prepare: async (source, settings) => editMaskInput(source, settings) },
    report,
    legacy: { read: async () => null },
    refreshPreviews: (sources) => previews.refreshSources(sources),
    verifyFile: async () => true,
    verifiedFile: () => true,
    files: {
      interpret: async (source) => source,
      load: async () => {
        throw new Error('No file source is configured.');
      },
    },
  });
  await binding.initialize();
  const card = document.createElement('article');
  card.className = 'sc-node-card';
  card.dataset.scTheme = 'gimp';
  const title = document.createElement('h2');
  title.textContent = 'Load Image';
  const fields = document.createElement('article');
  fields.className = 'lg-node';
  const view = new MasonryInputEditor(
    document,
    binding,
    previews,
    async () => {
      throw new Error('No upload fixture.');
    },
    report,
    nativeInputFields(node),
    fields,
    null,
  );
  card.append(title, fields, view.element, view.controlsElement);
  document.body.append(card);
  const source = nativeSourceField(node);
  const row = document.createElement('div');
  row.dataset.scStyleRow = '';
  const trigger = document.createElement('button');
  trigger.type = 'button';
  trigger.textContent = 'Current Composition';
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-label', source.name);
  trigger.setAttribute('aria-expanded', 'false');
  trigger.addEventListener('click', () => {
    trigger.setAttribute('aria-expanded', 'true');
  });
  row.append(trigger);
  fields.append(row);
  view.refresh();
  window.captureOnlyProof = {
    edit: () => {
      revision++;
    },
    read: () => ({ captures, authored: JSON.stringify(binding.state), errors: [...errors] }),
    dispose: () => {
      view.dispose();
      binding.dispose();
      previews.dispose();
      card.remove();
    },
  };
}

void mount().catch((error: unknown) => {
  console.error('Capture-only fixture could not mount', { error });
  document.body.textContent = 'Capture-only fixture could not mount.';
});
