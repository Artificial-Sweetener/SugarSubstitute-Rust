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
/** Prove an editor is told which LoRAs shape its prompt once per catalog and upstream change. */
import { expect, test } from '@jest/globals';
import { PromptLoraScheduling } from '../../../frontend/comfyui/ui/prompt_editor/runtime/PromptLoraScheduling.js';
import {
  preparePromptLoraCatalog,
  promptLoraItem,
} from '../../../frontend/comfyui/ui/prompt_editor/PromptLoraCatalog.js';
import { promptUpstreamLoras } from '../../../frontend/comfyui/ui/prompt_editor/PromptControlSupport.js';
import type { LoraSchedulingHost } from '../../../frontend/sugarprompt/editor/src/lora-schedule.js';
import type { LoraCatalogItem } from '../../../frontend/sugarprompt/services/src/index.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { promptCatalogFixture } from '../support/PromptCatalogFixture.js';

test('publishes the upstream LoRAs for a catalog once, and again only when either changes', () => {
  const catalog = preparePromptLoraCatalog(promptCatalogFixture(), 'http://localhost/');
  const example = promptLoraItem(catalog, 'folder/example.safetensors');
  if (!example) throw new Error('Missing catalog fixture');
  const published: LoraSchedulingHost[] = [];
  let upstream: readonly LoraCatalogItem[] = [];
  const scheduling = new PromptLoraScheduling(
    { setLoraSchedulingHost: (host) => published.push(host) },
    true,
    () => upstream,
  );

  scheduling.publish(catalog);
  scheduling.publish(catalog);
  expect(published).toHaveLength(1);
  expect(published[0]).toMatchObject({
    catalogRevision: catalog.revision,
    context: { catalogRevision: catalog.revision, mode: 'workflow', supported: true, upstream: [] },
  });

  upstream = [example];
  scheduling.publish(catalog);
  expect(published).toHaveLength(2);
  expect(published[1]?.context?.upstream.map((item) => item.model.backendValue)).toEqual([
    'folder/example.safetensors',
  ]);
  // Each publication is a new revision of the same catalog.
  expect(published[1]?.context?.revision).not.toBe(published[0]?.context?.revision);

  scheduling.reset();
  scheduling.publish(catalog);
  expect(published).toHaveLength(3);
});

test('a prompt that loads no written LoRAs is published as unsupported', () => {
  const catalog = preparePromptLoraCatalog(promptCatalogFixture(), 'http://localhost/');
  const published: LoraSchedulingHost[] = [];
  new PromptLoraScheduling(
    { setLoraSchedulingHost: (host) => published.push(host) },
    false,
    () => [],
  ).publish(catalog);
  expect(published[0]?.context?.supported).toBe(false);
});

test('only installed LoRAs configured ahead of the encoder count as upstream', () => {
  const catalog = preparePromptLoraCatalog(promptCatalogFixture(), 'http://localhost/');
  const graph: ComfyGraph = { _nodes: [] };
  const node = (id: string, type: string, widgets: ComfyNode['widgets'] = []): ComfyNode => {
    const member: ComfyNode = { id, type, graph, widgets };
    graph._nodes!.push(member);
    return member;
  };
  const prompt = node('prompt', 'PrimitiveStringMultiline');
  node('encode', 'PCTextEncode');
  node('lora', 'LoraLoader', [{ name: 'lora_name', value: 'folder/example.safetensors' }]);
  node('checkpoint', 'CheckpointLoaderSimple', [{ name: 'ckpt_name', value: 'base.safetensors' }]);
  graph.links = [
    { id: 1, origin_id: 'prompt', origin_slot: 0, target_id: 'encode', target_slot: 0 },
    { id: 2, origin_id: 'lora', origin_slot: 0, target_id: 'encode', target_slot: 1 },
    { id: 3, origin_id: 'checkpoint', origin_slot: 0, target_id: 'lora', target_slot: 0 },
  ];
  expect(promptUpstreamLoras(prompt, catalog).map((item) => item.model.backendValue)).toEqual([
    'folder/example.safetensors',
  ]);
  expect(promptUpstreamLoras(prompt, null)).toEqual([]);
});
