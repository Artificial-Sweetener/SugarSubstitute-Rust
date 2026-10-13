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
/** Prove source discovery, scoped metadata, and browser view lifetime against real application owners. */

import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { jest } from '@jest/globals';
import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { ComfyCanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasSourceInventory.js';
import { CanvasGraphFixture } from '../support/CanvasGraphFixture.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

/** Create colliding workflow IDs and distinct nested instance paths. */
function fixture() {
  const sessions = new WorkflowSessionStore();
  const results = new WorkflowResultStore();
  const inventory = new CanvasSourceInventory();
  const graph = new CanvasGraphFixture();
  const nested = new CanvasGraphFixture();
  nested.add({ id: 23, title: 'Inside', type: 'PreviewImage' });
  graph.add({ id: 1, title: '<b>Output</b>', type: 'PreviewImage' });
  graph.add({ id: 7, title: 'First nested', type: 'Nested', subgraph: nested });
  graph.add({ id: 8, title: 'Second nested', type: 'Nested', subgraph: nested });
  const a = { sessionId: workflowSessionId('a'), title: 'A', path: '', generation: 0 };
  const b = { ...a, sessionId: workflowSessionId('b'), title: 'B' };
  results.synchronize([a, b]);
  sessions.synchronize([a, b], a.sessionId);
  const errors: unknown[] = [];
  const adapter = new ComfyCanvasSourceInventory(sessions, inventory, () => graph, {
    error: (...args: unknown[]) => {
      errors.push(args);
    },
  });
  let key = 0;
  const views = new CanvasViewRegistry(() => String(++key));
  const selection = new CanvasSelectionController(views);
  const coordinator = new CanvasSessionCoordinator(
    sessions,
    results,
    inventory,
    views,
    selection,
    adapter,
  );
  adapter.install();
  coordinator.install();
  return {
    sessions,
    results,
    inventory,
    graph,
    nested,
    a,
    b,
    adapter,
    views,
    selection,
    coordinator,
    errors,
  };
}

describe('canvas source and workflow lifetimes', () => {
  test('reuses unchanged catalogs and refreshes only the workflow whose metadata changed', () => {
    const f = fixture();
    const first = f.coordinator.active();
    if (!first) throw new Error('Missing initial canvas view');
    const catalog = f.coordinator.catalog(first);
    expect(f.coordinator.catalog(first)).toBe(catalog);
    const reconcile = jest.spyOn(f.selection, 'reconcile');
    f.sessions.synchronize([f.a, f.b], f.b.sessionId);
    const second = f.coordinator.active();
    expect(second).not.toBe(first);
    expect(reconcile.mock.calls.every(([view]) => view === second)).toBe(true);
    reconcile.mockClear();
    f.sessions.synchronize([f.a, f.b], f.b.sessionId);
    expect(reconcile).not.toHaveBeenCalled();
    f.adapter.dispose();
    f.coordinator.dispose();
    reconcile.mockRestore();
  });
  test('preserves output batches and manual selection across native history restoration', () => {
    const f = fixture();
    const stop = f.sessions.subscribe((snapshot) => f.results.synchronize(snapshot.open));
    const source = f.inventory.read(f.a)[0];
    const media = decodeComfyMedia({
      images: [
        { filename: 'one.png', type: 'output' },
        { filename: 'two.png', type: 'output' },
      ],
    });
    if (!source || !media) throw new Error('Missing history canvas fixture');
    f.results.restore(f.a, source, media.payload, 'before-undo');
    const view = f.coordinator.active();
    if (!view) throw new Error('Missing history canvas view');
    f.selection.manual(view);
    const before = f.views.read(view);
    const outputs = f.results.read(f.a.sessionId)?.outputs;
    for (let index = 0; index < 4; index++) {
      const token = f.sessions.beginLoad('history');
      expect(f.coordinator.active()).toBe(view);
      expect(f.sessions.read().active).toBeNull();
      f.sessions.completeLoad(token, true);
      expect(f.coordinator.active()).toBe(view);
      expect(f.results.read(f.a.sessionId)?.outputs).toBe(outputs);
      expect(f.coordinator.catalog(view)?.entries[0]?.items).toHaveLength(2);
      expect(f.views.read(view).selection).toEqual(before.selection);
      expect(f.views.read(view).follow).toBe('manual');
    }
    stop();
    f.adapter.dispose();
    f.coordinator.dispose();
  });
  test('presents single Cube outputs by source name and keeps exact binding identity through caption changes', async () => {
    const f = fixture();
    const node = f.graph._nodes[1]!;
    node.title = 'Anima/Text to Image';
    node.properties = { sugarcubes_kind: 'cube', sugarcubes_cube: { target_model: 'Anima' } };
    f.nested.outputs.push({ name: 'image' });
    f.graph.afterChange(node);
    await Promise.resolve();
    const source = f.inventory
      .read(f.a)
      .find((source) => source.executionPath === buildCubeOutputExecutionId('7', 0));
    expect(source?.label).toBe('Text to Image');
    const serializations = f.graph.serializations;
    node.title = '<b>Literal source</b>';
    f.graph.afterChange(node);
    await Promise.resolve();
    expect(f.graph.serializations).toBe(serializations);
    expect(f.inventory.read(f.a).find((value) => value.id === source?.id)?.label).toBe(
      '<b>Literal source</b>',
    );
    f.adapter.dispose();
    f.coordinator.dispose();
  });
  test('discovers replaced nested instances and undo membership reported through generic completion', async () => {
    const f = fixture();
    const replacement = new CanvasGraphFixture();
    replacement.add({ id: 99, title: 'Replaced', type: 'PreviewImage' });
    f.graph._nodes[1]!.subgraph = replacement;
    const count = f.graph.serializations;
    f.graph.afterChange(f.graph._nodes[1]);
    await Promise.resolve();
    expect(f.graph.serializations).toBe(count + 1);
    expect(f.inventory.read(f.a).map((source) => source.executionPath)).toContain('7:99');
    expect(f.inventory.read(f.a).map((source) => source.executionPath)).not.toContain('7:23');
    f.graph._nodes.splice(0, 1, { id: 2, title: 'Undo restored', type: 'PreviewImage' });
    f.graph.afterChange();
    await Promise.resolve();
    expect(f.inventory.read(f.a)[0]?.executionPath).toBe('2');
    f.adapter.dispose();
    f.coordinator.dispose();
  });

  test('discovers exposure membership changes but refreshes output captions without serializing', async () => {
    const f = fixture();
    f.graph._nodes[1]!.properties = { sugarcubes_kind: 'cube' };
    f.nested.outputs.push({ name: 'One' });
    f.graph.afterChange(f.graph._nodes[1]);
    await Promise.resolve();
    const before = f.inventory.read(f.a).length;
    const count = f.graph.serializations;
    f.nested.outputs.push({ name: 'Two' });
    f.nested.afterChange();
    await Promise.resolve();
    expect(f.graph.serializations).toBe(count + 1);
    expect(f.inventory.read(f.a).length).toBe(before + 1);
    const output = f.nested.outputs[0]!;
    output.name = '<b>Literal rename</b>';
    f.nested.afterChange();
    await Promise.resolve();
    expect(f.graph.serializations).toBe(count + 1);
    expect(f.inventory.read(f.a).some((source) => source.breadcrumb.includes(output.name))).toBe(
      true,
    );
    f.nested.outputs.reverse();
    f.nested.afterChange();
    await Promise.resolve();
    expect(f.graph.serializations).toBe(count + 2);
    expect(
      f.inventory
        .read(f.a)
        .filter((source) => source.breadcrumb.includes('First nested'))
        .map((source) => source.label),
    ).toEqual(['First nested · Two', 'First nested · <b>Literal rename</b>', 'Inside']);
    f.adapter.dispose();
    f.coordinator.dispose();
  });
  test('discovers full nested instance paths once and never serializes for scoped values or unrelated UI', async () => {
    const f = fixture();
    expect(f.inventory.read(f.a).map((source) => source.executionPath)).toEqual([
      '1',
      '7',
      '7:23',
      '8',
      '8:23',
    ]);
    const serializations = f.graph.serializations;
    for (let index = 0; index < 100; index++) f.graph.afterChange(f.graph._nodes[0]);
    await Promise.resolve();
    expect(f.graph.serializations).toBe(serializations);
    const rootNode = f.graph._nodes[0];
    if (!rootNode) throw new Error('Fixture output is missing.');
    rootNode.title = '<script>literal</script>';
    f.graph.afterChange(rootNode);
    await Promise.resolve();
    expect(f.inventory.read(f.a)[0]?.label).toBe('<script>literal</script>');
    const nestedNode = f.nested._nodes[0];
    if (!nestedNode) throw new Error('Fixture nested node is missing.');
    nestedNode.title = 'Renamed';
    f.nested.afterChange(nestedNode);
    await Promise.resolve();
    expect(
      f.inventory
        .read(f.a)
        .filter((source) => source.executionPath.endsWith(':23'))
        .map((source) => source.label),
    ).toEqual(['Renamed', 'Renamed']);
    expect(f.graph.serializations).toBe(serializations);
    f.adapter.dispose();
    f.coordinator.dispose();
    f.graph.afterChange(rootNode);
    await Promise.resolve();
    expect(f.graph.serializations).toBe(serializations);
    expect(f.errors).toEqual([]);
  });

  test('coalesces genuine topology changes and removes deleted paths', async () => {
    const f = fixture();
    const count = f.graph.serializations;
    const extra = { id: 9, title: 'Another', type: 'PreviewImage' };
    f.graph.add(extra);
    f.graph.afterChange(extra);
    await Promise.resolve();
    expect(f.graph.serializations).toBe(count + 1);
    expect(f.inventory.read(f.a).at(-1)?.executionPath).toBe('9');
    f.graph.remove(extra);
    await Promise.resolve();
    expect(f.inventory.read(f.a).some((source) => source.executionPath === '9')).toBe(false);
    f.adapter.dispose();
    f.coordinator.dispose();
  });

  test('preserves view identity and Manual intent on tab return but retires closure and replacement generations', () => {
    const f = fixture();
    const first = f.coordinator.active();
    if (!first) throw new Error('Active view is missing.');
    f.selection.manual(first);
    f.sessions.synchronize([f.a, f.b], f.b.sessionId);
    expect(f.coordinator.active()).not.toBe(first);
    f.sessions.synchronize([{ ...f.a, title: 'Renamed A' }, f.b], f.a.sessionId);
    expect(f.coordinator.active()).toBe(first);
    expect(f.views.read(first).follow).toBe('manual');
    const token = f.sessions.beginLoad();
    expect(f.coordinator.active()).toBeNull();
    f.results.synchronize([{ ...f.a, generation: 1 }, f.b]);
    f.sessions.completeLoad(token, true);
    expect(f.coordinator.active()).toBe(first);
    expect(f.views.read(first).follow).toBe('auto');
    const retired: unknown[] = [];
    const stop = f.coordinator.onRetired((view) => retired.push(view));
    f.sessions.synchronize([f.b], f.b.sessionId);
    expect(retired).toEqual([first]);
    expect(f.inventory.read(f.a)).toEqual([]);
    expect(() => f.views.read(first)).toThrow('unavailable');
    f.sessions.synchronize([f.a, f.b], f.a.sessionId);
    expect(f.coordinator.active()).not.toBe(first);
    stop();
    f.adapter.dispose();
    f.coordinator.dispose();
  });

  test('clears stale metadata and reports a graph boundary failure', () => {
    const f = fixture();
    f.graph.serialize = () => {
      throw new Error('Serialization failed');
    };
    f.adapter.topologyChanged();
    expect(f.inventory.read(f.a)).toEqual([]);
    expect(f.errors).toHaveLength(1);
    f.adapter.dispose();
    f.coordinator.dispose();
  });
});
