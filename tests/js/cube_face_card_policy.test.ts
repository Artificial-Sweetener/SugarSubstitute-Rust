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
} from '../../frontend/comfyui/ui/surface/CubeFaceCardPolicy.js';
import {
  setCubeFaceCardRevealed,
  setCubeFaceNodeEnabled,
} from '../../frontend/comfyui/ui/surface/CubeFaceCardStateController.js';
import { createDefaultCubeSurfaceState } from '../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

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
