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
/** Prove prompt editors report separator hover to the regional owner and show its linked hover. */
import { expect, jest, test } from '@jest/globals';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import type { SugarPromptMountOptions } from '../../../frontend/comfyui/ui/prompt_editor/runtime/SugarPromptRuntime.js';
import type { PromptRegionLinks } from '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';

const editors: RecordedPromptEditor[] = [];

/** Stand in for the bundled engine, recording its regional hover traffic. */
class RecordedPromptEditor {
  readonly element = document.createElement('div');
  readonly highlighted: (number | null)[] = [];

  /** Retain mount options so the test can play the engine's pointer reports. */
  constructor(
    _document: Document,
    readonly options: SugarPromptMountOptions,
  ) {
    editors.push(this);
  }

  /** Place the editor beside its native textarea. */
  attach(textarea: HTMLTextAreaElement): void {
    textarea.after(this.element);
  }

  /** Record a linked highlight. */
  highlightRegion(ordinal: number | null): void {
    this.highlighted.push(ordinal);
  }

  /** Accept host value refreshes. */
  refresh(): void {}

  /** Accept catalog publication. */
  setLoraCatalog(): void {}

  /** Report immediate presentation. */
  async whenPresented(): Promise<void> {}

  /** Release one native document. */
  dispose(): void {
    this.element.remove();
  }
}

jest.unstable_mockModule(
  '../../../frontend/comfyui/ui/prompt_editor/runtime/SugarPromptRuntime.js',
  () => ({ SugarPromptEditor: RecordedPromptEditor }),
);
const { CubePromptEditorHost } = await import(
  '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js'
);

/** Mount one prompt node through the renderer-facing surface contract. */
function mount(host: InstanceType<typeof CubePromptEditorHost>, id: number): ComfyNode {
  const widget: ComfyWidget = { name: 'value', value: `prompt ${String(id)}` };
  const node: ComfyNode = { id, graph: { _nodes: [] }, widgets: [widget] };
  node.graph?._nodes?.push(node);
  const textarea = document.createElement('textarea');
  document.body.append(textarea);
  host.mount({
    node,
    widget,
    textarea,
    geometryChanged: () => undefined,
    hostReadOnly: () => false,
  });
  return node;
}

/** Let dynamic editor construction settle. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 5; turn++) await new Promise((resolve) => setTimeout(resolve, 0));
}

test('separator hover reaches the regional owner and its highlights reach only related editors', async () => {
  const report = jest.fn();
  const host = new CubePromptEditorHost(
    document,
    {
      load: () => new Promise(() => undefined),
      watch: () => () => undefined,
      prioritize: async () => undefined,
    },
    report,
    new AppearanceState(),
  );
  const hoverPrompt = jest.fn<(node: ComfyNode, ordinal: number | null) => void>();
  const release = jest.fn();
  let linked: Parameters<PromptRegionLinks['connectPrompts']>[0] | null = null;
  const regions: PromptRegionLinks = {
    hoverPrompt,
    connectPrompts: (editors) => {
      linked = editors;
      return release;
    },
  };
  host.setup(regions);
  host.setup(regions);
  try {
    const positive = mount(host, 1),
      negative = mount(host, 2);
    await settle();
    const [first, second] = editors;
    const prompts = linked as Parameters<PromptRegionLinks['connectPrompts']>[0] | null;
    if (!first || !second || !prompts) throw new Error('Two linked editors are required.');
    first.options.regionHover(1);
    first.options.regionHover(null);
    expect(hoverPrompt.mock.calls).toEqual([
      [positive, 1],
      [positive, null],
    ]);
    prompts.highlightRegions(new Set([negative]), 0);
    expect(first.highlighted).toEqual([]);
    expect(second.highlighted).toEqual([0]);
    hoverPrompt.mockImplementationOnce(() => {
      throw new Error('regional owner failed');
    });
    second.options.regionHover(2);
    expect(report).toHaveBeenCalledWith(new Error('regional owner failed'));
  } finally {
    host.dispose();
    document.body.replaceChildren();
  }
  expect(release).toHaveBeenCalledTimes(1);
});
