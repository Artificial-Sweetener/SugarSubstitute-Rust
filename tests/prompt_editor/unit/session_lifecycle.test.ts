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
/** Prove prompt editor sessions follow their workflow and node lifetimes across graph replacement. */
import { expect, jest, test } from '@jest/globals';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import type { PromptEditorSurface } from '../../../frontend/comfyui/ui/prompt_editor/PromptEditorSurface.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';

const liveEditors = new Set<object>();

/** Stand in for the bundled engine; construction and disposal are the observed lifetime. */
class RecordedPromptEditor {
  readonly element = document.createElement('div');

  /** Count one constructed native document. */
  constructor() {
    liveEditors.add(this);
  }

  /** Place the editor beside its native textarea. */
  attach(textarea: HTMLTextAreaElement): void {
    textarea.after(this.element);
  }

  /** Accept host value refreshes. */
  refresh(): void {}

  /** Accept catalog publication. */
  setLoraCatalog(): void {}

  /** Report immediate presentation. */
  async whenPresented(): Promise<void> {}

  /** Release one native document. */
  dispose(): void {
    liveEditors.delete(this);
    this.element.remove();
  }
}

jest.unstable_mockModule(
  '../../../frontend/comfyui/ui/prompt_editor/runtime/SugarPromptRuntime.js',
  () => ({
    SugarPromptEditor: RecordedPromptEditor,
  }),
);
const { CubePromptEditorHost } = await import(
  '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js'
);

interface MountedPrompt {
  readonly surface: PromptEditorSurface;
  release: (() => void) | null;
}

/** Create a host bound to an inert catalog so only session lifetime is exercised. */
function promptHost(): InstanceType<typeof CubePromptEditorHost> {
  return new CubePromptEditorHost(
    document,
    {
      load: () => new Promise(() => undefined),
      watch: () => () => undefined,
      prioritize: async () => undefined,
    },
    (error) => {
      throw error;
    },
    new AppearanceState(),
  );
}

/** Mount one prompt node of a graph through the renderer-facing surface contract. */
function mountPrompt(
  host: InstanceType<typeof CubePromptEditorHost>,
  graph: { _nodes: ComfyNode[] },
  id: number,
): MountedPrompt {
  const widget: ComfyWidget = { name: 'text', value: `prompt ${String(id)}` };
  const node: ComfyNode = { id, graph, widgets: [widget] };
  graph._nodes.push(node);
  const textarea = document.createElement('textarea');
  document.body.append(textarea);
  const surface: PromptEditorSurface = {
    node,
    widget,
    textarea,
    geometryChanged: () => undefined,
    hostReadOnly: () => false,
  };
  return { surface, release: host.mount(surface) };
}

/** Let dynamic editor construction settle. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 5; turn++) await new Promise((resolve) => setTimeout(resolve, 0));
}

test('returns to the live editor count after repeated workflow replacement and late re-renders', async () => {
  const host = promptHost();
  host.setup();
  const replaced: ComfyWidget[] = [];
  let mounted: MountedPrompt[] = [];
  try {
    for (let cycle = 0; cycle < 12; cycle++) {
      const graph = { _nodes: [] as ComfyNode[] };
      mounted = [1, 2, 3].map((id) => mountPrompt(host, graph, id));
      await settle();
      expect(liveEditors.size).toBe(3);

      host.beforeConfigureGraph();
      expect(liveEditors.size).toBe(0);
      // Deferred card teardown can re-render the replaced workflow before it unmounts.
      for (const prompt of mounted) expect(host.mount(prompt.surface)).toBeNull();
      graph._nodes.length = 0;
      for (const prompt of mounted) {
        prompt.release?.();
        prompt.surface.textarea.remove();
        replaced.push(prompt.surface.widget);
      }
      await settle();
      expect(liveEditors.size).toBe(0);
    }
    for (const widget of replaced)
      expect(Object.getOwnPropertyDescriptor(widget, 'value')).toMatchObject({ writable: true });
  } finally {
    host.dispose();
    document.body.replaceChildren();
  }
  expect(liveEditors.size).toBe(0);
});

test('retains an editor through renderer remounts but releases it when its node leaves the graph', async () => {
  const host = promptHost();
  host.setup();
  try {
    const graph = { _nodes: [] as ComfyNode[] };
    const prompt = mountPrompt(host, graph, 7);
    await settle();
    expect(liveEditors.size).toBe(1);
    prompt.release?.();
    const remount = host.mount(prompt.surface);
    await settle();
    expect(liveEditors.size).toBe(1);
    graph._nodes.length = 0;
    remount?.();
    expect(liveEditors.size).toBe(0);
    expect(Object.getOwnPropertyDescriptor(prompt.surface.widget, 'value')).toMatchObject({
      writable: true,
    });
    const restored = host.mount(prompt.surface);
    expect(restored).not.toBeNull();
    restored?.();
  } finally {
    host.dispose();
    document.body.replaceChildren();
  }
  expect(liveEditors.size).toBe(0);
});

test('restores a retained prompt once its workflow finishes configuring while replaced prompts stay native', async () => {
  const host = promptHost();
  host.setup();
  try {
    const graph = { _nodes: [] as ComfyNode[] };
    const retained = mountPrompt(host, graph, 1);
    const replaced = mountPrompt(host, graph, 2);
    await settle();
    expect(liveEditors.size).toBe(2);

    host.beforeConfigureGraph();
    expect(liveEditors.size).toBe(0);
    // Configuration drops the replaced node; the retained node keeps its exact graph lifetime.
    graph._nodes.splice(graph._nodes.indexOf(replaced.surface.node), 1);
    expect(host.mount(retained.surface)).toBeNull();
    host.afterConfigureGraph();

    expect(host.mount(replaced.surface)).toBeNull();
    const restored = host.mount(retained.surface);
    expect(restored).not.toBeNull();
    await settle();
    expect(liveEditors.size).toBe(1);
    restored?.();
  } finally {
    host.dispose();
    document.body.replaceChildren();
  }
  expect(liveEditors.size).toBe(0);
});
