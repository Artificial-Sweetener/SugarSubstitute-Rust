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
/** Verify the shared SugarSubstitute-compatible Cube card policy. */

import { jest } from '@jest/globals';

import {
  resolveCubeFaceCardPresentation,
  type CubeFaceCardDecision,
} from '../../../frontend/comfyui/ui/surface/CubeFaceCardPolicy.js';
import {
  setCubeFaceCardRevealed,
  setCubeFaceNodeEnabled,
} from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateController.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('CubeFaceCardPolicy', () => {
  test('hides authored bypass cards until reveal without enabling them', () => {
    const state = createDefaultCubeSurfaceState();
    const bypassed = node('mahiro', 'MahiroCFG', {
      mode: 4,
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });

    expect(resolveCubeFaceCardPresentation([bypassed], state)).toEqual({
      cards: [
        expect.objectContaining({
          id: 'mahiro',
          visible: false,
          enabled: false,
          showActivationControl: true,
        }),
      ],
      menuEntries: [{ id: 'mahiro', label: 'MahiroCFG', revealed: false }],
    });

    setCubeFaceCardRevealed(state, bypassed, true);

    expect(decision([bypassed], state, 'mahiro')).toEqual(
      expect.objectContaining({ visible: true, enabled: false }),
    );
    expect(bypassed.mode).toBe(4);
  });

  test('hiding a revealed bypass card disables it and preserves its activation choice', () => {
    const state = createDefaultCubeSurfaceState();
    const bypassed = node('patch', 'Patch', { mode: 4, widgets: [] });
    setCubeFaceCardRevealed(state, bypassed, true);

    setCubeFaceNodeEnabled(state, bypassed, true);
    expect(decision([bypassed], state, 'patch')).toEqual(
      expect.objectContaining({ visible: true, enabled: true, showActivationControl: true }),
    );

    setCubeFaceCardRevealed(state, bypassed, false);
    expect(decision([bypassed], state, 'patch')).toEqual(
      expect.objectContaining({ visible: false, enabled: false }),
    );
    expect(bypassed.mode).toBe(4);

    setCubeFaceCardRevealed(state, bypassed, true);
    expect(decision([bypassed], state, 'patch')).toEqual(
      expect.objectContaining({ visible: true, enabled: true }),
    );
    expect(bypassed.mode).toBe(0);
  });

  test('keeps a normally visible transform out of the reveal menu while sharing activation', () => {
    const state = createDefaultCubeSurfaceState();
    const patch = node('patch', 'UnregisteredModelPatch', {
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });

    const presentation = resolveCubeFaceCardPresentation([patch], state);

    expect(presentation.cards).toEqual([
      expect.objectContaining({
        id: 'patch',
        visible: true,
        enabled: true,
        showActivationControl: true,
      }),
    ]);
    expect(presentation.menuEntries).toEqual([]);
  });

  test('does not turn a disabled Mahiro activation control into a reveal-menu entry', () => {
    const state = createDefaultCubeSurfaceState();
    const mahiro = node('mahiro', 'Mahiro', {
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });

    setCubeFaceNodeEnabled(state, mahiro, false);
    const presentation = resolveCubeFaceCardPresentation([mahiro], state);

    expect(mahiro.mode).toBe(4);
    expect(presentation.cards).toEqual([
      expect.objectContaining({
        id: 'mahiro',
        visible: true,
        enabled: false,
        showActivationControl: true,
      }),
    ]);
    expect(presentation.menuEntries).toEqual([]);
  });

  test('shows the SDXL Mahiro card without treating its subgraph worker as a patch', () => {
    const state = createDefaultCubeSurfaceState();
    const mahiro = node('mahiro', 'Mahiro', {
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });
    const schedule = node('schedule', 'ScheduleAndEncodeSubgraph', {
      inputs: [{ type: 'MODEL' }, { type: 'CLIP' }],
      outputs: [{ type: 'MODEL' }, { type: 'CONDITIONING' }],
      isSubgraphNode: () => true,
    });

    expect(resolveCubeFaceCardPresentation([mahiro, schedule], state).cards).toEqual([
      expect.objectContaining({
        id: 'mahiro',
        showActivationControl: true,
      }),
    ]);
  });

  test('hard-hides Schedule & Encode and leaves host-owned Vectorscope controls unchanged', () => {
    const state = createDefaultCubeSurfaceState();
    const mahiro = node('mahiro', 'MahiroCFG', {
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });
    const schedule = node('schedule', 'SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl', {
      widgets: [{ name: 'encode_style' }],
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });
    const vectorscope = node('scope', 'VectorscopeCC', {
      widgets: [{ name: 'brightness' }],
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
    });

    const presentation = resolveCubeFaceCardPresentation([mahiro, schedule, vectorscope], state);

    expect(
      presentation.cards.map(({ id, showActivationControl }) => ({
        id,
        showActivationControl,
      })),
    ).toEqual([
      { id: 'mahiro', showActivationControl: true },
      { id: 'scope', showActivationControl: false },
    ]);
    expect(presentation.menuEntries).toEqual([]);
  });

  test('does not offer ordinary widget cards through the reveal menu', () => {
    const state = createDefaultCubeSurfaceState();
    const titleOnly = node('decode', 'VAEDecode');
    const controlled = node('sampler', 'KSampler', {
      widgets: [{ name: 'steps' }],
      isWidgetVisible: jest.fn(() => true),
    });

    const presentation = resolveCubeFaceCardPresentation([titleOnly, controlled], state);

    expect(presentation.cards.map(({ id }) => id)).toEqual(['sampler']);
    expect(presentation.menuEntries).toEqual([]);
  });

  test('omits a node whose only widget is consumed by an incoming graph connection', () => {
    const state = createDefaultCubeSurfaceState();
    const promptEncoder = node('encode', 'CLIPTextEncode', {
      widgets: [{ name: 'text', type: 'customtext' }],
      inputs: [{ name: 'text', link: 41, widget: { name: 'text' } }],
      outputs: [{ type: 'CONDITIONING' }],
    });

    expect(resolveCubeFaceCardPresentation([promptEncoder], state)).toEqual({
      cards: [],
      menuEntries: [],
    });
  });

  test('retains a card when an unconsumed sibling widget remains editable', () => {
    const state = createDefaultCubeSurfaceState();
    const mixed = node('mixed', 'PromptStyler', {
      widgets: [
        { name: 'text', type: 'customtext' },
        { name: 'strength', value: 0.5 },
      ],
      inputs: [{ name: 'text', link: 41, widget: { name: 'text' } }],
    });

    expect(resolveCubeFaceCardPresentation([mixed], state).cards).toEqual([
      expect.objectContaining({ id: 'mixed', visible: true }),
    ]);
  });

  test('keeps an authored-bypass sampler revealable without adding an activation switch', () => {
    const state = createDefaultCubeSurfaceState();
    const sampler = node('sampler', 'KSampler', {
      mode: 4,
      widgets: [{ name: 'steps' }, { name: 'denoise' }],
    });

    const presentation = resolveCubeFaceCardPresentation([sampler], state);

    expect(presentation.cards).toEqual([
      expect.objectContaining({
        id: 'sampler',
        visible: false,
        enabled: false,
        showActivationControl: false,
      }),
    ]);
    expect(presentation.menuEntries).toEqual([
      { id: 'sampler', label: 'KSampler', revealed: false },
    ]);
  });

  test('double-spans exactly one semantic prompt editor while keeping ordinary multiline text narrow', () => {
    const state = createDefaultCubeSurfaceState();
    const prompt = node('positive', 'CLIPTextEncode', {
      widgets: [{ name: 'text', type: 'customtext' }],
      title: 'Positive prompt',
    });
    const note = node('notes', 'Notes', {
      widgets: [{ name: 'text', type: 'customtext' }],
      title: 'Notes',
    });

    const cards = resolveCubeFaceCardPresentation([prompt, note], state).cards;

    expect(cards.map(({ id, columnSpan }) => ({ id, columnSpan }))).toEqual([
      { id: 'positive', columnSpan: 2 },
      { id: 'notes', columnSpan: 1 },
    ]);
  });

  test('uses CONDITIONING flow to recognize an unlabeled standard Comfy prompt editor', () => {
    const state = createDefaultCubeSurfaceState();
    const prompt = node('encode', 'CLIPTextEncode', {
      widgets: [{ name: 'text', type: 'customtext' }],
      outputs: [{ type: 'CONDITIONING', links: ['conditioning-link'] }],
    });
    const sampler = node('sampler', 'KSampler', {
      inputs: [{ name: 'positive', type: 'CONDITIONING', link: 'conditioning-link' }],
    });
    const graph = {
      _nodes: [prompt, sampler],
      links: {
        'conditioning-link': {
          id: 'conditioning-link',
          origin_id: 'encode',
          target_id: 'sampler',
          target_slot: 0,
        },
      },
    };
    prompt.graph = graph;
    sampler.graph = graph;

    expect(decision([prompt], state, 'encode').columnSpan).toBe(2);
  });

  test('does not promote an ambiguous pair of multiline fields into prompt cards', () => {
    const state = createDefaultCubeSurfaceState();
    const ambiguous = node('prompt', 'PromptPair', {
      title: 'Prompt pair',
      widgets: [
        { name: 'first', type: 'customtext' },
        { name: 'second', type: 'customtext' },
      ],
    });

    expect(decision([ambiguous], state, 'prompt').columnSpan).toBe(1);
  });

  test('pins positive and negative prompts above every ordinary masonry card', () => {
    const state = createDefaultCubeSurfaceState();
    const sampler = node('sampler', 'KSampler', {
      inputs: [{ name: 'model', type: 'MODEL', link: 'scheduled-model-link' }],
      widgets: [{ name: 'steps' }],
    });
    const negative = node('negative', 'CLIPTextEncode', {
      title: 'Negative prompt',
      widgets: [{ name: 'text', type: 'customtext' }],
    });
    const models = node('models', 'Models', {
      outputs: [{ name: 'model', type: 'MODEL', links: ['model-link'] }],
      widgets: [{ name: 'model_name' }],
    });
    const schedule = node('schedule', 'SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl', {
      inputs: [{ name: 'model', type: 'MODEL', link: 'model-link' }],
      outputs: [{ name: 'model', type: 'MODEL', links: ['scheduled-model-link'] }],
    });
    const positive = node('positive', 'CLIPTextEncode', {
      title: 'Positive prompt',
      widgets: [{ name: 'text', type: 'customtext' }],
    });
    const graph = {
      _nodes: [sampler, negative, models, positive, schedule],
      links: {
        'model-link': {
          id: 'model-link',
          origin_id: 'models',
          target_id: 'schedule',
          target_slot: 0,
        },
        'scheduled-model-link': {
          id: 'scheduled-model-link',
          origin_id: 'schedule',
          target_id: 'sampler',
          target_slot: 0,
        },
      },
    };
    expect(
      resolveCubeFaceCardPresentation(
        [sampler, negative, models, positive, schedule],
        state,
        graph,
      ).cards.map(({ id }) => id),
    ).toEqual(['positive', 'negative', 'models', 'sampler']);
  });
});

/** Build one narrowly typed graph node fixture. */
function node(id: string, type: string, overrides: Partial<ComfyNode> = {}): ComfyNode {
  return { id, type, mode: 0, widgets: [], inputs: [], outputs: [], ...overrides };
}

/** Resolve one expected card by stable node id. */
function decision(
  nodes: readonly ComfyNode[],
  state: ReturnType<typeof createDefaultCubeSurfaceState>,
  id: string,
): CubeFaceCardDecision {
  const card = resolveCubeFaceCardPresentation(nodes, state).cards.find(
    (candidate) => candidate.id === id,
  );
  if (!card) throw new Error(`Missing Cube card '${id}'.`);
  return card;
}
