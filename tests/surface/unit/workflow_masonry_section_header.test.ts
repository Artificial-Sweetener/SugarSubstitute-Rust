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
/** Verify masonry shares Cube identity rendering and keeps loose headings literal. */
import { describe, expect, jest, test } from '@jest/globals';
import { WorkflowMasonrySectionHeaderView } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonrySectionHeaderView.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

describe('masonry section identity', () => {
  test('shows the shared Cube icon and model pill without a count, and retains action identity', () => {
    const cube = cubeFixture();
    const section: WorkflowMasonrySection = {
      id: 'cube:example',
      label: 'SDXL/Text to Image',
      cube,
      target: cube,
      cards: [],
    };
    const openCube = jest.fn();
    const view = new WorkflowMasonrySectionHeaderView(section, {
      document,
      cardState: cardState(),
      openCube,
    });
    view.render(section);
    const image = view.element.querySelector('img');
    expect(image?.getAttribute('src')).toBe('/sugarcubes/assets/icon?cube_id=example');
    expect(view.element.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('SDXL');
    expect(view.element.querySelector('.sugarcubes-model-title__name')?.textContent).toBe(
      'Text to Image',
    );
    expect(view.element.querySelector('small')).toBeNull();
    expect(view.element.textContent).not.toMatch(/\d+ nodes/);
    const action = view.element.querySelector('button');
    expect(action?.textContent).toBe('');
    expect(action?.querySelector('i')?.classList.contains('icon-[comfy--workflow]')).toBe(true);
    expect(action?.querySelector('i')?.getAttribute('aria-hidden')).toBe('true');
    expect(action?.getAttribute('type')).toBe('button');
    expect(action?.title).toBe('Open SDXL/Text to Image in graph');
    expect(action?.getAttribute('aria-label')).toBe('Open SDXL/Text to Image');
    view.render(section);
    expect(view.element.querySelector('img')).toBe(image);
    expect(view.element.querySelector('button')).toBe(action);
    action?.click();
    expect(openCube).toHaveBeenCalledWith(cube);
    section.label = '<img src=x> My Cube';
    view.render(section);
    expect(view.element.querySelector('h2')?.textContent).toContain('<img src=x> My Cube');
    expect(view.element.querySelectorAll('img')).toHaveLength(1);
    expect(view.element.querySelector('[data-sugarcubes-model-pill]')).toBeNull();
    expect(action?.getAttribute('aria-label')).toBe('Open <img src=x> My Cube');
    view.dispose();
  });
  test('keeps loose section headings as literal text without Cube chrome or counts', () => {
    const section: WorkflowMasonrySection = {
      id: 'loose:1',
      label: '<b>Workflow nodes</b>',
      cube: null,
      target: { _nodes: [] },
      cards: [],
    };
    const view = new WorkflowMasonrySectionHeaderView(section, {
      document,
      cardState: cardState(),
      openCube() {},
    });
    view.render(section);
    expect(view.element.querySelector('h2')?.textContent).toBe('<b>Workflow nodes</b>');
    expect(
      view.element.querySelector('b,img,canvas,small,[data-sugarcubes-model-pill]'),
    ).toBeNull();
    view.dispose();
  });
});

/** Bind the existing optional-node presentation owner without browser or graph services. */
function cardState(): CubeFaceCardStateService {
  return new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} });
}

/** Supply a typed native Cube with an authoritative model route and asset icon. */
function cubeFixture(): CubeNode {
  return {
    id: 'cube',
    title: 'SDXL/Text to Image',
    pos: [0, 0],
    size: [360, 200],
    inputs: [],
    outputs: [],
    widgets: [],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        default_alias: 'SDXL/Text to Image',
        target_model: 'SDXL',
        cube_id: 'example',
        icon: {
          kind: 'asset',
          url: '/sugarcubes/assets/icon?cube_id=example',
          media_type: 'image/svg+xml',
        },
      },
    },
    subgraph: {
      id: 'definition',
      name: 'SDXL/Text to Image',
      _nodes: [],
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
    connect() {},
    isSubgraphNode: () => true,
    serialize: () => ({}),
  };
}
