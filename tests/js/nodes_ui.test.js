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
import { describe, expect, test, beforeEach, jest } from '@jest/globals';
import { app } from './mocks/app.js';
import { getGroupSugarcubes } from '../../web/comfyui/ui/graph/GroupMetadata.js';
import { getSugarCubesUI } from '../../web/comfyui/ui/index.js';

beforeEach(() => {
  app.reset();
  globalThis.LiteGraph = {
    INPUT: 1,
    OUTPUT: 2,
    LGraphCanvas: { node_colors: { IMAGE: { color: '#111', bgcolor: '#222' } } },
  };
  Object.assign(globalThis.window, {
    SugarCubes: {},
    setInterval: () => 1,
    clearInterval: () => {},
    comfyAPI: { vueApp: { config: { globalProperties: { $toast: null } } } },
  });
});

describe('nodes ui integration', () => {
  const loadNodesUi = () => {
    const cacheBust = `?v=${Math.random().toString(36).slice(2)}`;
    return import(`../../web/js/nodes.js${cacheBust}`);
  };

  test('registers extension and updates node titles', async () => {
    await loadNodesUi();
    expect(app._extensions).toHaveLength(1);
    const extension = app._extensions[0];

    class NodeType {
      constructor() {
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null, links: [] }];
        this.outputs = [{ name: 'value', links: [] }];
        this.widgets = [{ name: 'default_alias', value: 'demo' }];
        this.graph = {
          links: {
            1: { id: 1, type: 'IMAGE', origin_id: 1, target_id: 2, origin_slot: 0, target_slot: 0 },
          },
        };
        this.id = 1;
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(NodeType, { name: 'SugarCubes.CubeInput' });
    const node = new NodeType();
    node.outputs[0].links = [1];
    node.graph.getNodeById = () => null;

    extension.nodeCreated(node);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(node.title).toBe('IMAGE Input');
    expect(node.outputs[0].label).toBe('IMAGE');
  });

  test('adopts cube metadata for connected markers', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];

    const graph = {
      _nodes: [],
      _groups: [],
      getNodeById(id) {
        return this._nodes.find((node) => node?.id === id) || null;
      },
    };
    const internalNode = {
      id: 10,
      type: 'KSampler',
      pos: [0, 0],
      size: [100, 50],
      inputs: [],
      outputs: [{ type: 'IMAGE', links: [] }],
      graph,
    };
    const group = {
      title: 'Demo',
      properties: {
        sugarcubes: {
          managed: true,
          instance_id: 'inst_demo',
          cube_id: 'local/example-user/demo.cube',
          default_alias: 'Demo',
          instance_alias: 'Demo',
          markers: { inputs: [], outputs: [] },
          nodes: [internalNode.id],
        },
      },
    };
    graph._nodes.push(internalNode);
    graph._groups.push(group);

    class MarkerType {
      constructor() {
        this.id = 11;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: 'IMAGE', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: '' },
          { name: 'default_alias', value: '' },
          { name: 'instance_alias', value: '' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = graph;
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const marker = new MarkerType();
    graph._nodes.push(marker);
    extension.nodeCreated(marker);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const metadata = getGroupSugarcubes(group);
    metadata.markers.inputs.push(marker.id);

    marker.onConnectionsChange(1, 0, true, {
      origin_id: marker.id,
      origin_slot: 0,
      target_id: internalNode.id,
      target_slot: 0,
      type: 'IMAGE',
    });

    const cubeId = marker.widgets.find((widget) => widget.name === 'cube_id')?.value;
    const defaultAlias = marker.widgets.find((widget) => widget.name === 'default_alias')?.value;
    const alias = marker.widgets.find((widget) => widget.name === 'instance_alias')?.value;
    const instanceId = marker.widgets.find((widget) => widget.name === 'instance_id')?.value;
    expect(cubeId).toBe('local/example-user/demo.cube');
    expect(defaultAlias).toBe('Demo');
    expect(alias).toBe('Demo');
    expect(instanceId).toBe('inst_demo');
  });

  test('does not sync alias when default_alias widget changes', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];

    class MarkerType {
      constructor() {
        this.id = 20;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: '*', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: '' },
          { name: 'default_alias', value: 'Alpha' },
          { name: 'instance_alias', value: 'Alpha' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = { _groups: [], _nodes: [this], links: {} };
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const marker = new MarkerType();
    const defaultAliasWidget = marker.widgets.find((widget) => widget.name === 'default_alias');
    defaultAliasWidget.value = 'Beta';

    marker.onWidgetChanged('default_alias', 'Beta', 'Alpha', defaultAliasWidget);

    const alias = marker.widgets.find((widget) => widget.name === 'instance_alias')?.value;
    expect(alias).toBe('Alpha');
  });

  test('shows cube name and hides alias for uninitialized markers', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];

    class MarkerType {
      constructor() {
        this.id = 30;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: '*', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: '' },
          { name: 'default_alias', value: 'Draft Cube' },
          { name: 'instance_alias', value: 'Draft Cube' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = { _groups: [], _nodes: [this], links: {} };
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const marker = new MarkerType();

    extension.nodeCreated(marker);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const defaultAlias = marker.widgets.find((widget) => widget.name === 'default_alias');
    const alias = marker.widgets.find((widget) => widget.name === 'instance_alias');
    expect(defaultAlias.hidden).toBe(false);
    expect(defaultAlias.disabled).toBe(false);
    expect(defaultAlias.options.hidden).toBe(false);
    expect(alias.hidden).toBe(true);
    expect(alias.disabled).toBe(true);
    expect(alias.options.hidden).toBe(true);
  });

  test('shows alias and hides cube name for initialized markers', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];

    class MarkerType {
      constructor() {
        this.id = 31;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: '*', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: 'local/personal/draft_cube.cube' },
          { name: 'default_alias', value: 'Draft Cube' },
          { name: 'instance_alias', value: 'Draft Cube' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = { _groups: [], _nodes: [this], links: {} };
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const marker = new MarkerType();

    extension.nodeCreated(marker);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const defaultAlias = marker.widgets.find((widget) => widget.name === 'default_alias');
    const alias = marker.widgets.find((widget) => widget.name === 'instance_alias');
    expect(defaultAlias.hidden).toBe(true);
    expect(defaultAlias.disabled).toBe(true);
    expect(defaultAlias.options.hidden).toBe(true);
    expect(alias.hidden).toBe(false);
    expect(alias.disabled).toBe(false);
    expect(alias.options.hidden).toBe(false);
  });

  test('exposes create action only for uninitialized markers', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];
    const handler = jest.fn();
    getSugarCubesUI().cubeCreation.startCreateCubeFromMarker = handler;

    class MarkerType {
      constructor({ cubeId }) {
        this.id = cubeId ? 41 : 40;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: '*', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: cubeId },
          { name: 'default_alias', value: 'Draft Cube' },
          { name: 'instance_alias', value: 'Draft Cube' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = { _groups: [], _nodes: [this], links: {} };
      }
      addWidget(type, name, value, callback, options = {}) {
        const widget = { type, name, value, callback, options };
        this.widgets.push(widget);
        return widget;
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const draftMarker = new MarkerType({ cubeId: '' });
    const savedMarker = new MarkerType({ cubeId: 'local/personal/draft_cube.cube' });

    extension.nodeCreated(draftMarker);
    extension.nodeCreated(savedMarker);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const draftAction = draftMarker.widgets.find((widget) => widget.name === 'Create cube');
    const savedAction = savedMarker.widgets.find((widget) => widget.name === 'Create cube');
    expect(draftAction.hidden).toBe(false);
    expect(draftAction.disabled).toBe(false);
    expect(draftAction.options.serialize).toBe(false);
    expect(savedAction.hidden).toBe(true);
    expect(savedAction.disabled).toBe(true);

    draftAction.callback();
    expect(handler).toHaveBeenCalledWith(draftMarker);

    const draftMenu = draftMarker.getExtraMenuOptions();
    const savedMenu = savedMarker.getExtraMenuOptions();
    expect(draftMenu.some((entry) => entry.content === 'Create SugarCube')).toBe(true);
    expect(savedMenu.some((entry) => entry.content === 'Create SugarCube')).toBe(false);
  });

  test('create action uses current ui instance at click time', async () => {
    await loadNodesUi();
    const extension = app._extensions[0];
    const staleHandler = jest.fn();
    const currentHandler = jest.fn();
    getSugarCubesUI().cubeCreation.startCreateCubeFromMarker = staleHandler;
    getSugarCubesUI({ forceNew: true }).cubeCreation.startCreateCubeFromMarker = currentHandler;

    class MarkerType {
      constructor() {
        this.id = 50;
        this.type = 'SugarCubes.CubeInput';
        this.title = 'Cube Input';
        this.inputs = [{ name: 'value', link: null }];
        this.outputs = [{ type: '*', links: [] }];
        this.widgets = [
          { name: 'cube_id', value: '' },
          { name: 'default_alias', value: 'Draft Cube' },
          { name: 'instance_alias', value: 'Draft Cube' },
          { name: 'instance_id', value: '' },
        ];
        this.graph = { _groups: [], _nodes: [this], links: {} };
      }
      addWidget(type, name, value, callback, options = {}) {
        const widget = { type, name, value, callback, options };
        this.widgets.push(widget);
        return widget;
      }
      setDirtyCanvas() {}
      update() {}
    }

    extension.beforeRegisterNodeDef(MarkerType, { name: 'SugarCubes.CubeInput' });
    const marker = new MarkerType();
    extension.nodeCreated(marker);
    await new Promise((resolve) => setTimeout(resolve, 0));

    marker.widgets.find((widget) => widget.name === 'Create cube').callback();

    expect(staleHandler).not.toHaveBeenCalled();
    expect(currentHandler).toHaveBeenCalledWith(marker);
  });
});
