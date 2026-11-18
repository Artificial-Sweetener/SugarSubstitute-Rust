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
/** Prove LoRA names are offered only where a prompt's text reaches a Prompt Control encoder. */
import { expect, test } from '@jest/globals';
import {
  feedsPromptControl,
  promptUpstreamValues,
} from '../../../frontend/comfyui/ui/prompt_editor/PromptControlSupport.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Build one graph from `[origin, target]` connections between typed nodes. */
function graph(
  types: Record<string, string>,
  connections: readonly [string, string][],
): ComfyGraph {
  const members: ComfyGraph = { _nodes: [] };
  for (const [id, type] of Object.entries(types))
    members._nodes!.push({ id, type, graph: members } as ComfyNode);
  members.links = connections.map(([origin, target], index) => ({
    id: index + 1,
    origin_id: origin,
    origin_slot: 0,
    target_id: target,
    target_slot: 0,
  }));
  return members;
}

/** Read one fixture node by identity. */
function node(members: ComfyGraph, id: string): ComfyNode {
  const found = members._nodes?.find((member) => member.id === id);
  if (!found) throw new Error(`Missing fixture node ${id}`);
  return found;
}

test.each([
  'SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl',
  'PCLazyTextEncode',
  'PCLazyTextEncodeAdvanced',
  'PCTextEncode',
])('a prompt wired to %s loads the LoRAs written in it', (encoder) => {
  const members = graph({ prompt: 'PrimitiveStringMultiline', encode: encoder }, [
    ['prompt', 'encode'],
  ]);
  expect(feedsPromptControl(node(members, 'prompt'))).toBe(true);
});

test('text that passes through other nodes on its way to the encoder still counts', () => {
  const members = graph(
    { prompt: 'PrimitiveStringMultiline', join: 'StringConcatenate', encode: 'PCTextEncode' },
    [
      ['prompt', 'join'],
      ['join', 'encode'],
    ],
  );
  expect(feedsPromptControl(node(members, 'prompt'))).toBe(true);
});

test('a prompt that reaches only an ordinary encoder is not offered LoRA names', () => {
  const members = graph(
    { prompt: 'PrimitiveStringMultiline', encode: 'CLIPTextEncode', other: 'PCTextEncode' },
    [['prompt', 'encode']],
  );
  expect(feedsPromptControl(node(members, 'prompt'))).toBe(false);
});

test('a title that names Prompt Control is not an encoder, and a loop of nodes ends', () => {
  const members = graph({ prompt: 'PrimitiveStringMultiline', note: 'Note' }, [
    ['prompt', 'note'],
    ['note', 'prompt'],
  ]);
  node(members, 'note').title = 'PCTextEncode';
  expect(feedsPromptControl(node(members, 'prompt'))).toBe(false);
  expect(feedsPromptControl({ id: 'loose', type: 'PrimitiveStringMultiline' })).toBe(false);
});

test('models configured on the nodes feeding the encoder are found; the text path is not searched', () => {
  const members = graph(
    {
      prompt: 'PrimitiveStringMultiline',
      join: 'StringConcatenate',
      encode: 'PCTextEncode',
      lora: 'LoraLoader',
      checkpoint: 'CheckpointLoaderSimple',
      unrelated: 'LoraLoader',
    },
    [
      ['prompt', 'join'],
      ['join', 'encode'],
      ['checkpoint', 'lora'],
      ['lora', 'encode'],
    ],
  );
  node(members, 'prompt').widgets = [{ name: 'value', value: 'styles/ink.safetensors' }];
  node(members, 'join').widgets = [{ name: 'delimiter', value: ', ' }];
  node(members, 'lora').widgets = [
    { name: 'lora_name', value: 'styles/ink.safetensors' },
    { name: 'strength_model', value: 0.8 },
  ];
  node(members, 'checkpoint').widgets = [{ name: 'ckpt_name', value: 'base.safetensors' }];
  node(members, 'unrelated').widgets = [{ name: 'lora_name', value: 'styles/pastel.safetensors' }];
  // The loader ahead of the encoder and everything ahead of it; never the prompt's own text.
  expect(promptUpstreamValues(node(members, 'prompt'))).toEqual([
    'styles/ink.safetensors',
    'base.safetensors',
  ]);
});

test('a prompt that reaches no Prompt Control encoder has no upstream models', () => {
  const members = graph({ prompt: 'PrimitiveStringMultiline', lora: 'LoraLoader' }, [
    ['lora', 'prompt'],
  ]);
  node(members, 'lora').widgets = [{ name: 'lora_name', value: 'styles/ink.safetensors' }];
  expect(promptUpstreamValues(node(members, 'prompt'))).toEqual([]);
});
