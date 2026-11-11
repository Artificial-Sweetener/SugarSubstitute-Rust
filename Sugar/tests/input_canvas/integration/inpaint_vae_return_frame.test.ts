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
/** Prove only a verified VAE round trip projects Comfy's center crop into return coordinates. */
import { hasCompiledVaeCenterCrop } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyVaeReturnFrame.js';

const image = { classType: 'LoadImage', field: 'image', files: ['captured.png'] };
const prompt = {
  image: { class_type: 'LoadImage', inputs: { image: 'captured.png' } },
  encode: {
    class_type: 'InpaintModelConditioning',
    inputs: { pixels: ['image', 0], vae: ['vae', 0] },
  },
  sample: { class_type: 'SimpleSyrup.KSamplerExtras', inputs: { latent_image: ['encode', 2] } },
  decode: { class_type: 'VAEDecode', inputs: { samples: ['sample', 0], vae: ['vae', 0] } },
  save: { class_type: 'SugarCubes.CubeOutput', inputs: { value: ['decode', 0] } },
};

test('recognizes the failed production encode/sample/decode path', () => {
  expect(hasCompiledVaeCenterCrop(prompt, 'save', image)).toBe(true);
});

test('does not infer a crop through resizing, unknown samplers, different sources or VAE changes', () => {
  expect(
    hasCompiledVaeCenterCrop(
      {
        ...prompt,
        sample: { class_type: 'LatentUpscaleBy', inputs: { samples: ['encode', 2], scale_by: 2 } },
      },
      'save',
      image,
    ),
  ).toBe(false);
  expect(
    hasCompiledVaeCenterCrop(
      { ...prompt, sample: { ...prompt.sample, class_type: 'UnknownSampler' } },
      'save',
      image,
    ),
  ).toBe(false);
  expect(hasCompiledVaeCenterCrop(prompt, 'save', { ...image, files: ['other.png'] })).toBe(false);
  expect(
    hasCompiledVaeCenterCrop(
      {
        ...prompt,
        decode: { ...prompt.decode, inputs: { samples: ['sample', 0], vae: ['other-vae', 0] } },
      },
      'save',
      image,
    ),
  ).toBe(false);
});

test('keeps full-frame composites and cyclic graphs out of the cropped-frame path', () => {
  expect(
    hasCompiledVaeCenterCrop(
      {
        ...prompt,
        composite: {
          class_type: 'ImageCompositeMasked',
          inputs: { source: ['decode', 0], destination: ['image', 0] },
        },
        save: { class_type: 'SaveImage', inputs: { images: ['composite', 0] } },
      },
      'save',
      image,
    ),
  ).toBe(false);
  expect(
    hasCompiledVaeCenterCrop(
      { ...prompt, sample: { ...prompt.sample, inputs: { latent_image: ['sample', 0] } } },
      'save',
      image,
    ),
  ).toBe(false);
});
