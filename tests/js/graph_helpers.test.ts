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
import { describe, expect, test } from '@jest/globals';
import {
  buildLinkIndex,
  collectGraphLinks,
  getGraphGroups,
  getGraphNodes,
} from '../../web/comfyui/ui/graph/GraphQuery.js';
import {
  getNodeCenter,
  isPointInBounds,
  readGroupBounds,
  readNodeBounds,
} from '../../web/comfyui/ui/graph/Bounds.js';
import { readWidgetValue, writeWidgetValue } from '../../web/comfyui/ui/graph/Markers.js';
import { coerceVec2, readVector2 } from '../../web/comfyui/ui/graph/VectorUtils.js';
import {
  readCubeMarkerInstanceId,
  updateMarkersForCubeId,
  writeCubeMarkerInstanceId,
} from '../../web/comfyui/ui/graph/CubeMarkers.js';
import { resolveInstanceDisplayName } from '../../web/comfyui/ui/graph/GroupMetadata.js';

describe('graph helpers', () => {
  test('getGraphNodes returns _nodes or nodes arrays', () => {
    expect(getGraphNodes(null)).toEqual([]);
    expect(getGraphNodes({ _nodes: [1, 2] })).toEqual([1, 2]);
    expect(getGraphNodes({ nodes: [3] })).toEqual([3]);
    expect(getGraphNodes({ nodes: 'nope' })).toEqual([]);
  });

  test('collectGraphLinks supports arrays, maps, and objects', () => {
    const map = new Map([
      ['a', { id: 1 }],
      ['b', null],
    ]);
    expect(collectGraphLinks({ links: [{ id: 1 }, null] })).toEqual([{ id: 1 }]);
    expect(collectGraphLinks({ links: map })).toEqual([{ id: 1 }]);
    expect(collectGraphLinks({ links: { one: { id: 2 }, two: null } })).toEqual([{ id: 2 }]);
    expect(collectGraphLinks({ links: null, _links: { one: { id: 3 }, two: null } })).toEqual([
      { id: 3 },
    ]);
  });

  test('collectGraphLinks reconstructs links from node slots when link tables are unavailable', () => {
    const source = {
      id: 10,
      outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [42] }],
      inputs: [],
    };
    const target = {
      id: 11,
      outputs: [],
      inputs: [{ name: 'image', type: 'IMAGE', link: 42 }],
    };
    const graph = {
      _nodes: [source, target],
      links: new Map(),
      _links: new Map(),
    };

    expect(collectGraphLinks(graph)).toEqual([
      {
        id: 42,
        origin_id: 10,
        origin_slot: 0,
        target_id: 11,
        target_slot: 0,
        type: 'IMAGE',
      },
    ]);
  });

  test('getGraphGroups returns _groups or groups arrays', () => {
    expect(getGraphGroups(null)).toEqual([]);
    expect(getGraphGroups({ _groups: ['a'] })).toEqual(['a']);
    expect(getGraphGroups({ groups: ['b'] })).toEqual(['b']);
    expect(getGraphGroups({ groups: 'nope' })).toEqual([]);
  });

  test('buildLinkIndex indexes by origin and target', () => {
    const graph = {
      links: [
        { origin_id: 1, target_id: 2, id: 'a' },
        { origin: 2, target: 3, id: 'b' },
      ],
    };
    const { outgoing, incoming, links } = buildLinkIndex(graph);
    expect(links).toHaveLength(2);
    expect(outgoing.get('1')).toHaveLength(1);
    expect(incoming.get('3')).toHaveLength(1);
  });

  test('readNodeBounds prefers getBounding then pos/size', () => {
    const node = {
      getBounding: () => [1, 2, 3, 4],
      pos: [10, 20],
      size: [30, 40],
    };
    expect(readNodeBounds(node)).toEqual([1, 2, 3, 4]);
    expect(readNodeBounds({ pos: [1, 2], size: [3, 4] })).toEqual([1, 2, 3, 4]);
    expect(
      readNodeBounds({
        pos: new Float32Array([5, 6]),
        size: new Float32Array([7, 8]),
      }),
    ).toEqual([5, 6, 7, 8]);
    expect(readNodeBounds({})).toBeNull();
  });

  test('readGroupBounds prefers pos/size over stale _bounding', () => {
    const group = { _bounding: [1, 2, 3, 4] };
    expect(readGroupBounds(group)).toEqual([1, 2, 3, 4]);
    expect(readGroupBounds({ _bounding: [1, 2, 3, 4], pos: [5, 6], size: [7, 8] })).toEqual([
      5, 6, 7, 8,
    ]);
    expect(readGroupBounds({ pos: [5, 6], size: [7, 8] })).toEqual([5, 6, 7, 8]);
    expect(readGroupBounds(null)).toBeNull();
  });

  test('bounds helpers compute centers and containment', () => {
    const node = { pos: [10, 20], size: [30, 40] };
    expect(getNodeCenter(node)).toEqual([25, 40]);
    expect(isPointInBounds([5, 5], [0, 0, 10, 10])).toBe(true);
    expect(isPointInBounds([15, 5], [0, 0, 10, 10])).toBe(false);
  });

  test('read/write widget values handle callbacks safely', () => {
    const widget = {
      name: 'default_alias',
      value: 'demo',
      callback: () => {
        throw new Error('nope');
      },
    };
    const node = { widgets: [widget] };
    expect(readWidgetValue(node, 'default_alias')).toBe('demo');
    expect(writeWidgetValue(node, 'default_alias', 'next')).toBe(true);
    expect(widget.value).toBe('next');
  });

  test('read/write widget values support legacy cube_name default alias widgets', () => {
    const widget = { name: 'cube_name', value: 'Legacy Demo' };
    const node = { widgets: [widget] };
    expect(readWidgetValue(node, 'default_alias')).toBe('Legacy Demo');
    expect(writeWidgetValue(node, 'default_alias', 'Updated Demo')).toBe(true);
    expect(widget.value).toBe('Updated Demo');
  });

  test('vector helpers coerce and default values', () => {
    expect(readVector2(null, 1, 2)).toEqual([1, 2]);
    expect(readVector2(['3', '4'])).toEqual([3, 4]);
    expect(readVector2(['x', 2], 9, 9)).toEqual([9, 2]);
    expect(coerceVec2([1, 2])).toEqual([1, 2]);
    expect(coerceVec2([1, 'x'])).toBeNull();
  });

  test('updateMarkersForCubeId updates cube markers only', () => {
    const marker = {
      type: 'SugarCubes.CubeInput',
      widgets: [
        { name: 'cube_id', value: 'old' },
        { name: 'default_alias', value: 'Old Name' },
        { name: 'instance_id', value: '' },
      ],
    };
    const other = {
      type: 'Other.Node',
      widgets: [
        { name: 'cube_id', value: 'old' },
        { name: 'default_alias', value: 'Old Name' },
      ],
    };
    const graph = { _nodes: [marker, other] };
    expect(updateMarkersForCubeId(graph, 'old', { cubeId: 'new', defaultAlias: 'New Name' })).toBe(
      1,
    );
    expect(readWidgetValue(marker, 'cube_id')).toBe('new');
    expect(readWidgetValue(marker, 'default_alias')).toBe('New Name');
  });

  test('cube marker instance_id helpers read and write', () => {
    const marker = {
      type: 'SugarCubes.CubeInput',
      widgets: [{ name: 'instance_id', value: '' }],
    };
    expect(readCubeMarkerInstanceId(marker)).toBe('');
    expect(writeCubeMarkerInstanceId(marker, 'inst-1')).toBe(true);
    expect(readCubeMarkerInstanceId(marker)).toBe('inst-1');
  });

  test('resolveInstanceDisplayName prefers custom alias then default alias', () => {
    const metadata = { instance_alias: 'Alias Name', default_alias: 'Canonical Name' };
    const group = { title: 'Group Title' };
    expect(resolveInstanceDisplayName({ metadata, group, fallback: 'Fallback' })).toBe(
      'Alias Name',
    );
    expect(
      resolveInstanceDisplayName({
        metadata: {
          instance_alias: 'Canonical Name',
          default_alias: 'Canonical Name',
        },
      }),
    ).toBe('Canonical Name');
    expect(
      resolveInstanceDisplayName({
        metadata: { instance_alias: '', default_alias: 'Canonical Name' },
      }),
    ).toBe('Canonical Name');
    expect(
      resolveInstanceDisplayName({ metadata: { instance_alias: '' }, group, fallback: 'Fallback' }),
    ).toBe('Group Title');
    expect(
      resolveInstanceDisplayName({ metadata: { instance_alias: '' }, fallback: 'Fallback' }),
    ).toBe('Fallback');
  });
});
