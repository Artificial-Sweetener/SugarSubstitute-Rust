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
/** Supply a deterministic installed LoRA with distinct standard and banner thumbnail choices. */
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';

/** Keep exact installed source spelling independent of literal display labels and media. */
export function promptCatalogFixture(): ModelCatalogSnapshot {
  return {
    revision: 1,
    scanning: false,
    fieldContracts: [],
    items: [
      {
        id: 'example-lora',
        kind: 'loras',
        value: 'folder/example.safetensors',
        rootId: 'models',
        relativePath: 'folder/example.safetensors',
        folder: 'folder',
        title: 'Example <b>model</b>',
        version: 'v2',
        baseModel: 'SDXL',
        provider: '',
        providerUrl: '',
        providerModelId: '',
        providerVersionId: '',
        providerLinks: [],
        description: '',
        trainedWords: ['example trigger'],
        thumbnails: [
          {
            assetId: 'standard',
            role: 'standard',
            width: 128,
            height: 128,
            url: '/proof/standard.svg',
          },
          { assetId: 'banner', role: 'banner', width: 512, height: 128, url: '/proof/banner.svg' },
        ],
      },
    ],
  };
}
