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
/** Prove Substitute label conventions and reversible native text ownership. */
import { afterEach, expect, test } from '@jest/globals';
import { formatFluentDisplayLabel } from '../../../frontend/comfyui/ui/surface/appearance/FluentDisplayLabels.js';
import { NativeLabelPresentation } from '../../../frontend/comfyui/ui/surface/appearance/NativeLabelPresentation.js';

afterEach(() => document.body.replaceChildren());

test.each([
  ['sampler_name', 'Sampler'],
  ['sampler name', 'Sampler'],
  ['scheduler', 'Scheduler'],
  ['cfg', 'CFG'],
  ['Cfg', 'CFG'],
  ['ksampler', 'KSampler'],
  ['vectorscopeCC', 'VectorscopeCC'],
  ['', ''],
  ['  ', ''],
  ['positive_prompt', 'Positive Prompt'],
  ['text_to_image', 'Text to Image'],
  ['image to image', 'Image to Image'],
  ['a_walk_in_the_park', 'A Walk in the Park'],
  ['before_and_after', 'Before and After'],
  ['the_lord_of_the_rings', 'The Lord of the Rings'],
  ['from_here_to', 'From Here To'],
  ['Vae', 'VAE'],
  ['Lora', 'LoRA'],
  ['vae_loader', 'VAE Loader'],
  ['lora_name', 'LoRA Name'],
  ['cfg_scale', 'CFG Scale'],
  ['clip_loader', 'Clip Loader'],
  ['CLIP_loader', 'CLIP Loader'],
  ['sdxl_model', 'Sdxl Model'],
  ['mahiro CFG', 'Mahiro CFG'],
  ['SDXL/text_to_image', 'SDXL/Text to Image'],
  ['sdxl/Text to Image', 'sdxl/Text to Image'],
  ['SDXL/vae_loader', 'SDXL/VAE Loader'],
  ['/Text to Image', '/Text to Image'],
  ['SDXL/', 'SDXL/'],
  ['ESRGAN Model', 'ESRGAN Model'],
  ['constructor', 'Constructor'],
  ['__proto__', 'Proto'],
])('formats %j as %j with Substitute conventions', (source, expected) => {
  expect(formatFluentDisplayLabel(source)).toBe(expected);
});

/** Mount literal text without parsing dynamic labels as markup. */
function fixture(source: string) {
  const root = document.createElement('div');
  const label = document.createElement('span');
  label.className = 'truncate';
  const text = document.createTextNode(source);
  label.append(text);
  root.append(label);
  document.body.append(root);
  const presentation = new NativeLabelPresentation();
  return { root, label, text, presentation };
}

test('restores latest native labels and preserves eliding classes and text-node identity', () => {
  const { root, label, text, presentation } = fixture('sampler_name');
  presentation.reconcile(root, [label]);
  expect(text.data).toBe('Sampler');
  presentation.reconcile(root, [label]);
  expect(label.firstChild).toBe(text);
  expect(label.className).toBe('truncate');
  text.data = 'vae_loader';
  presentation.reconcile(root, [label]);
  expect(text.data).toBe('VAE Loader');
  presentation.clear();
  expect(text.data).toBe('vae_loader');
});

test('does not overwrite host changes pending reconciliation when disabled', () => {
  const { root, label, text, presentation } = fixture('cfg_scale');
  presentation.reconcile(root, [label]);
  text.data = 'authored replacement';
  presentation.clear();
  expect(text.data).toBe('authored replacement');
});

test('releases replaced and detached labels without touching a native title editor', () => {
  const { root, label, text, presentation } = fixture('positive_prompt');
  presentation.reconcile(root, [label]);
  const input = document.createElement('input');
  input.value = 'positive_prompt';
  label.replaceChildren(input);
  input.focus();
  presentation.reconcile(root, [label]);
  expect(text.data).toBe('positive_prompt');
  expect(input.value).toBe('positive_prompt');
  expect(document.activeElement).toBe(input);
  label.replaceChildren(document.createTextNode('negative_prompt'));
  presentation.reconcile(root, [label]);
  expect(label.textContent).toBe('Negative Prompt');
  root.remove();
  const next = fixture('ksampler');
  presentation.reconcile(next.root, [next.label]);
  expect(label.textContent).toBe('negative_prompt');
  presentation.clear();
});

test('keeps markup-like labels literal and leaves custom nested content intact', () => {
  const { root, label, presentation } = fixture('<img src=x>');
  const custom = document.createElement('div');
  const child = document.createElement('span');
  child.textContent = 'custom_sTYle';
  custom.append(child);
  root.append(custom);
  presentation.reconcile(root, [label, custom]);
  expect(root.querySelector('img')).toBeNull();
  expect(child.textContent).toBe('custom_sTYle');
  presentation.clear();
  expect(label.textContent).toBe('<img src=x>');
});
