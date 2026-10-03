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
/** Prove Substitute's title-icon semantics without changing graph state or unknown headers. */
import { expect, test } from '@jest/globals';
import { resolveMasonryNodeIcon } from '../../../frontend/comfyui/ui/surface/masonry/MasonryNodeIconPolicy.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

test.each([
  ['LoadImage', 'folder'],
  ['LoadImageMask', 'folder'],
  ['SimpleSyrup.LoadMaskBatch', 'folder'],
  ['EmptyLatentImage', 'photo'],
  ['KSampler', 'application'],
  ['DetailerForEach', 'application'],
  ['SimpleSyrup.PromptEncodeStyle', 'edit'],
  ['VectorscopeCC', 'palette'],
  ['VAELoader', 'model'],
  ['CheckpointLoaderSimple', 'model'],
])('matches Substitute class %s with %s', (type, expected) => {
  expect(resolveMasonryNodeIcon({ type })).toBe(expected);
});

test('shares semantic prompt roles, including conditioning flow and ambiguous-role fallback', () => {
  const prompt: ComfyNode = {
    id: 1,
    title: 'Prompt',
    widgets: [{ name: 'text', type: 'customtext' }],
    outputs: [{ type: 'CONDITIONING', links: [10] }],
  };
  prompt.graph = {
    _nodes: [prompt, { id: 2, inputs: [{ name: 'negative' }] }],
    links: { '10': { id: 10, origin_id: 1, target_id: 2, target_slot: 0 } },
  };
  expect(resolveMasonryNodeIcon(prompt)).toBe('eraser');
  expect(resolveMasonryNodeIcon({ title: 'Positive prompt', widgets: prompt.widgets })).toBe(
    'edit',
  );
  expect(resolveMasonryNodeIcon({ title: 'Negative prompt', widgets: prompt.widgets })).toBe(
    'eraser',
  );
  expect(
    resolveMasonryNodeIcon({ title: 'Positive negative prompt', widgets: prompt.widgets }),
  ).toBe('edit');
});

test('infers sampler workers before model pickers and keeps model links and numeric strengths distinct', () => {
  expect(
    resolveMasonryNodeIcon({
      type: 'CustomSampler',
      widgets: [
        { name: 'steps', type: 'number' },
        { name: 'denoise', type: 'number' },
        { name: 'ckpt_name', type: 'combo' },
      ],
    }),
  ).toBe('application');
  expect(
    resolveMasonryNodeIcon({
      type: 'CustomLoader',
      widgets: [{ name: 'lora_name', type: 'combo' }],
    }),
  ).toBe('model');
  expect(
    resolveMasonryNodeIcon({
      type: 'UpscaleModelLoader',
      widgets: [{ name: 'model_name', type: 'combo' }],
    }),
  ).toBe('model');
  expect(
    resolveMasonryNodeIcon({
      type: 'Custom',
      inputs: [{ name: 'clip', type: 'CLIP' }],
      widgets: [{ name: 'lora_strength', type: 'number' }],
    }),
  ).toBeNull();
});

test('does not invent an icon for an unknown custom card or treat titles as URLs', () => {
  expect(
    resolveMasonryNodeIcon({ type: 'Custom', title: '<img src=x onerror=alert(1)>' }),
  ).toBeNull();
  expect(resolveMasonryNodeIcon({ type: 'Custom', title: 'Model Loader' })).toBeNull();
});
