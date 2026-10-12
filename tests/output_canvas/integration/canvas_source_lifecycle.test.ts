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
import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { ComfyCanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasSourceInventory.js';
import { CanvasGraphFixture } from '../support/CanvasGraphFixture.js';

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
