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
/** Prove shared section geometry and stable dependency-aware workflow projection. */
import { describe, expect, test } from '@jest/globals';
import { computeWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryLayout.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { resolveCubeCardWidgetSources } from '../../../frontend/comfyui/ui/surface/CubeCardWidgetSources.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import { resolveCubeFaceCardPresentation } from '../../../frontend/comfyui/ui/surface/CubeFaceCardPolicy.js';
import { projectNodeCards } from '../../../frontend/comfyui/ui/surface/NodeCardProjection.js';
import {
  readCubeFaceCardState,
  CubeFaceCardStateService,
} from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

describe('workflow masonry sections', () => {
  test('uses exact Cube hiding and activation decisions, including persisted optional-node choices', () => {
    const prompt = node('prompt', 0);
    const hidden = node('hidden', 0);
    hidden.type = 'SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl';
    const infrastructure = node('infrastructure', 0);
    infrastructure.widgets = [];
    const optional = node('optional', 0);
    optional.mode = 4;
    const transform = node('transform', 0);
    transform.widgets = [];
    transform.inputs = [{ type: 'MODEL' }];
    transform.outputs = [{ type: 'MODEL' }];
    const cube = cubeNode('cube', [prompt, hidden, infrastructure, optional, transform], 0);
    const graph = { _nodes: [cube] };
    const nodes = new CubeNodeCatalog();
    nodes.add(cube);
    const service = new CubeFaceCardStateService({ nodes, history: {} });
    const projection = () => projectWorkflowMasonry(graph, [cube])[0]!.cards;
    const decisions = () =>
      resolveCubeFaceCardPresentation(
        cube.subgraph._nodes,
        readCubeFaceCardState(cube),
        cube.subgraph,
      ).cards;
    expect(projection()).toEqual(
      decisions().map((card) => ({
        ...card,
        widgetSources: [],
        prompt: false,
        id: `cube:cube:${card.id}`,
        columnSpan: card.columnSpan ?? 1,
      })),
    );
    expect(
      projection()
        .filter((card) => card.visible)
        .map((card) => card.node.id),
    ).toEqual(['prompt', 'transform']);
    service.setRevealed(cube, 'optional', true);
    expect(projection().find((card) => card.node === optional)).toMatchObject({
      visible: true,
      enabled: false,
      showActivationControl: true,
    });
    service.setEnabled(cube, 'optional', true);
    expect(projection().find((card) => card.node === optional)).toMatchObject({
      visible: true,
      enabled: true,
    });
    service.setEnabled(cube, 'transform', false);
    expect(projection().find((card) => card.node === transform)).toMatchObject({
      visible: true,
      enabled: false,
    });
  });
  test('exposes forwarded boundary controls but never exposes values supplied by a graph link', () => {
    const member = node('prompt', 0);
    member.inputs = [{ name: 'text', widget: { name: 'text' }, link: 5 }];
    const cube = cubeNode('cube', [member], 0);
    cube.inputs = [{ name: 'prompt', widget: { name: 'prompt' }, link: null }];
    cube.subgraph.inputNode.id = -10;
    cube.subgraph.getLink = () => ({ origin_id: -10, origin_slot: 0 });
    expect(resolveCubeCardWidgetSources(cube, member)).toEqual([
      { node: cube, name: 'prompt', targetName: 'text' },
    ]);
    member.title = 'Positive Prompt';
    member.widgets = [{ name: 'text', value: 'original', type: 'customtext' }];
    const before = JSON.stringify(member);
    const faceCards = projectNodeCards(
      cube.subgraph._nodes,
      readCubeFaceCardState(cube),
      cube.subgraph,
      cube,
    );
    const workflowCards = projectWorkflowMasonry({ _nodes: [cube] }, [cube])[0]?.cards;
    expect(faceCards).toEqual([
      expect.objectContaining({
        node: member,
        columnSpan: 2,
        prompt: true,
        widgetSources: [{ node: cube, name: 'prompt', targetName: 'text' }],
      }),
    ]);
    expect(
      workflowCards?.map(({ id, ...card }) => ({ ...card, id: id.replace('cube:cube:', '') })),
    ).toEqual(faceCards);
    expect(JSON.stringify(member)).toBe(before);
    cube.inputs[0]!.link = 8;
    expect(resolveCubeCardWidgetSources(cube, member)).toEqual([]);
    cube.inputs[0]!.link = null;
    cube.subgraph.getLink = () => ({ origin_id: 'other-node', origin_slot: 0 });
    expect(resolveCubeCardWidgetSources(cube, member)).toEqual([]);
  });
  test('restarts below the tallest column and retains one shared width and span policy', () => {
    const result = computeWorkflowMasonry(
      [
        {
          id: 'first',
          cards: [
            { id: 'a', height: 100 },
            { id: 'b', height: 240 },
          ],
        },
        {
          id: 'second',
          cards: [
            { id: 'c', height: 60 },
            { id: 'prompt', height: 120, columnSpan: 2 },
          ],
        },
      ],
      { availableWidth: 980, minimumColumnWidth: 320, gap: 10 },
    );
    expect(result.placements.map((card) => [card.id, card.x, card.y, card.width])).toEqual([
      ['a', 0, 44, 320],
      ['b', 330, 44, 320],
      ['c', 0, 352, 320],
      ['prompt', 330, 352, 650],
    ]);
    expect(result.sections[1]?.y).toBe(308);
    expect(result.height).toBe(496);
  });

  test('clamps spans in narrow viewports and handles empty sections', () => {
    const result = computeWorkflowMasonry(
      [
        { id: 'empty', cards: [] },
        { id: 'cube', cards: [{ id: 'prompt', height: 80, columnSpan: 2 }] },
      ],
      { availableWidth: 220, minimumColumnWidth: 320, gap: 12 },
    );
    expect(result.placements[0]).toMatchObject({ width: 220, x: 0, y: 112 });
    expect(result.height).toBe(216);
  });

  test('projects Cube and loose runs without changing node geometry, values, or identity', () => {
    const first = node('before', -200);
    const inner = node('inside', 0);
    const cube = cubeNode('cube', [inner], 0);
    const after = node('after', 900);
    const graph: ComfyGraph = { _nodes: [after, cube, first] };
    const snapshot = JSON.stringify(graph);
    const sections = projectWorkflowMasonry(graph, [cube]);
    expect(
      sections.map((section) => [section.id, section.cards.map((card) => card.node.id)]),
    ).toEqual([
      ['loose:before', ['before']],
      ['cube:cube', ['inside']],
      ['loose:after', ['after']],
    ]);
    expect(sections[1]?.cards[0]?.node).toBe(inner);
    expect(JSON.stringify(graph)).toBe(snapshot);
  });

  test('respects effective proximity edges and applies shared card eligibility', () => {
    const source = cubeNode('source', [node('custom', 0)], 1000);
    const target = cubeNode('target', [node('widgetless', 0)], 0);
    const sections = projectWorkflowMasonry(
      { _nodes: [target, source] },
      [source, target],
      [{ source: 'source', target: 'target' }],
    );
    expect(sections.map((section) => section.id)).toEqual(['cube:source', 'cube:target']);
    expect(sections.flatMap((section) => section.cards)).toHaveLength(2);
  });

  test('orders linked branches and cycles deterministically and keeps later prompts in their Cube', () => {
    const upstream = node('upstream', 800);
    const downstream = node('downstream', 0);
    downstream.inputs = [{ name: 'input', link: 1 }];
    const prompt = node('prompt', 0);
    prompt.title = 'Positive prompt';
    prompt.widgets = [{ name: 'text', type: 'customtext' }];
    const cube = cubeNode('later', [prompt], 1400);
    cube.inputs = [{ name: 'input', link: 3 }];
    const graph: ComfyGraph = {
      _nodes: [downstream, cube, upstream],
      links: [
        { id: 1, origin_id: 'upstream', target_id: 'downstream' },
        { id: 3, origin_id: 'downstream', target_id: 'later' },
      ],
    };
    expect(
      projectWorkflowMasonry(graph, [cube]).flatMap((section) =>
        section.cards.map((card) => card.node.id),
      ),
    ).toEqual(['upstream', 'downstream', 'prompt']);
    upstream.inputs = [{ name: 'input', link: 2 }];
    graph.links = [
      { id: 1, origin_id: 'upstream' },
      { id: 2, origin_id: 'downstream' },
    ];
    expect(projectWorkflowMasonry(graph, [cube])).toEqual(projectWorkflowMasonry(graph, [cube]));
    expect(projectWorkflowMasonry(graph, [cube]).flatMap((section) => section.cards)).toHaveLength(
      3,
    );
  });
});

/** Construct a typed host node without weakening the graph boundary contract. */
function node(id: string, x: number): NativeGraphNode {
  return {
    id,
    type: 'Example',
    pos: [x, 0],
    size: [330, 140],
    properties: {},
    inputs: [],
    outputs: [],
    widgets: [{ name: 'value', type: 'number', value: 1 }],
    connect() {},
  };
}

/** Construct a real-subgraph-shaped Cube with deliberately separate member identity. */
function cubeNode(id: string, members: NativeGraphNode[], x: number): CubeNode {
  return {
    ...node(id, x),
    title: id,
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: id } },
    subgraph: {
      id: `definition-${id}`,
      name: id,
      _nodes: members,
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        return { connect() {} };
      },
      addOutput() {
        return { connect() {} };
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    serialize: () => ({}),
  };
}
