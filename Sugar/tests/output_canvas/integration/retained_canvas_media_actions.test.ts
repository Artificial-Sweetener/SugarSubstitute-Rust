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
/** Keep retained-only actions bound to verified artifact sources and accurate item capabilities. */
import { jest } from '@jest/globals';
import { desktopSessionFixture } from '../../workflow/support/desktop_session.js';
import { canvasSnapshot, canvasOperationScope } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { desktopOutputResultId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { CanvasMediaActions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasMediaActions.js';
import { retainedCanvasOutput } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/RetainedCanvasOutput.js';
import { retainedOutputCapabilities } from '../../../frontend/comfyui/ui/output_saving/BrowserRetainedOutputTransfer.js';
import { canvasContextActions } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.js';
import type { CanvasViewSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';

/** Feed actual owners immutable portable identity plus an explicitly retained-only presentation grant. */
function fixture(video = false) {
  const shared = desktopSessionFixture();
  const entry = shared.checkpoint.entries[0];
  const output = entry?.outputs[0];
  const catalog = canvasCatalogFixture(1, 1, undefined, 'retained-actions');
  const reference = catalog.entries[0]?.items[0];
  const source = catalog.entries[0]?.source;
  if (!entry || !output || !reference || !source) throw new Error('Missing output fixture.');
  const binding = {
    ...shared.fence.bindings[0]!,
    runtimeSession: reference.provenance.sessionId,
    generation: reference.provenance.generation,
  };
  const retained = {
    ...output,
    assetDigest: output.digest,
    ...(video
      ? {
          bytes: 70 * 1024 * 1024,
          media: 'video' as const,
          mime: 'video/mp4',
          file: { ...output.file, filename: 'large.mp4' },
        }
      : {}),
  };
  const item: WorkflowOutputItem = {
    ...reference,
    id: desktopOutputResultId(output.id),
    mediaKind: video ? 'video' : 'image',
    artifact: { output: retained, binding },
    presentationUrl: `${shared.origin}/__sugarsubstitute/session-media/v1/${'a'.repeat(32)}/${output.id}?binding=${encodeURIComponent(JSON.stringify(binding))}`,
    file: {
      filename: retained.file.filename,
      subfolder: retained.file.subfolder,
      type: retained.file.storage,
    },
  };
  const base = canvasSnapshot('retained-actions');
  let current: CanvasViewSnapshot = {
    ...base,
    selected: item,
    grid: [{ source, items: [item], preview: null, summary: catalog.entries[0]!.summary }],
    resource: { ...base.resource, state: 'ready' },
    mediaActions: { preferredCopyFormat: 'png', fileAction: 'reveal', photoshop: true },
  };
  const copy = jest.fn<() => Promise<boolean>>().mockResolvedValue(true);
  const file = jest.fn<() => Promise<boolean>>().mockResolvedValue(true);
  const photoshop = jest.fn<() => Promise<boolean>>().mockResolvedValue(true);
  const owner = new CanvasMediaActions({
    read: () => current,
    entries: () => current.grid,
    accepts: () => true,
    copy,
    fileAction: file,
    photoshop,
    profile: (item) => ({
      preferredCopyFormat: 'png',
      fileAction: retainedCanvasOutput(item) ? 'download' : 'reveal',
      photoshop: !retainedCanvasOutput(item),
    }),
    capabilities: (item) => {
      const source = retainedCanvasOutput(item);
      const facts = source ? retainedOutputCapabilities(source) : null;
      return {
        copy: facts?.copy ?? true,
        file: facts?.download ?? true,
        photoshop: facts?.photoshop ?? true,
        drag: facts?.drag ?? true,
      };
    },
  });
  current = {
    ...current,
    itemActions: owner.itemActions(),
    photoshopAllAvailable: owner.photoshopAllAvailable(),
  };
  return {
    item,
    owner,
    copy,
    file,
    photoshop,
    read: () => current,
    change: (value: CanvasViewSnapshot) => {
      current = value;
    },
  };
}

test('retained still offers exact copy/download and refuses Photoshop for the selected item or mixed all-output request', async () => {
  const f = fixture();
  const first = f.read().grid[0];
  if (!first) throw new Error('Missing current output entry.');
  const { presentationUrl: _url, artifact: _artifact, ...backend } = f.item;
  const other: WorkflowOutputItem = { ...backend, id: 'result:backend-original', batchIndex: 2 };
  f.change({ ...f.read(), grid: [{ ...first, items: [f.item, other] }] });
  f.change({
    ...f.read(),
    itemActions: f.owner.itemActions(),
    photoshopAllAvailable: f.owner.photoshopAllAvailable(),
  });
  const menu = canvasContextActions(
    f.read(),
    { schema: 1, commands: ['copy-media', 'file-media', 'photoshop-media', 'photoshop-all-media'] },
    () => {},
  );
  expect(menu.find((entry) => entry.label === 'Download')?.disabled).toBe(false);
  expect(menu.find((entry) => entry.label === 'Open in Photoshop')?.disabled).toBe(true);
  expect(menu.find((entry) => entry.label === 'Open All in Photoshop')?.disabled).toBe(true);
  expect(
    await f.owner.run({ type: 'photoshop-media', target: 'primary' }, canvasOperationScope()),
  ).toBe(false);
  expect(await f.owner.run({ type: 'photoshop-all-media' }, canvasOperationScope())).toBe(false);
  expect(f.photoshop).not.toHaveBeenCalled();
  expect(
    await f.owner.run(
      { type: 'copy-media', target: 'primary', format: 'preferred' },
      canvasOperationScope(),
    ),
  ).toBe(true);
  expect(await f.owner.run({ type: 'file-media', target: 'primary' }, canvasOperationScope())).toBe(
    true,
  );
  expect(f.copy).toHaveBeenCalledTimes(1);
  expect(f.file).toHaveBeenCalledTimes(1);
  expect(retainedCanvasOutput(f.item)).toMatchObject({
    output: { id: f.item.id },
    url: f.item.presentationUrl,
  });
  f.owner.dispose();
});

test('large retained video remains present while copy, drag and download are accurately unavailable', async () => {
  const f = fixture(true);
  const facts = f.owner.itemActions()[0];
  expect(facts).toMatchObject({
    copy: false,
    file: false,
    drag: false,
    photoshop: false,
    fileAction: 'download',
  });
  const menu = canvasContextActions(
    f.read(),
    { schema: 1, commands: ['copy-media', 'file-media'] },
    () => {},
  );
  expect(menu.find((entry) => entry.label === 'Download')?.disabled).toBe(true);
  expect(await f.owner.run({ type: 'file-media', target: 'primary' }, canvasOperationScope())).toBe(
    false,
  );
  expect(await f.owner.open(f.item, canvasOperationScope())).toBe(false);
  expect(f.file).not.toHaveBeenCalled();
  expect(f.copy).not.toHaveBeenCalled();
  f.owner.dispose();
});

test('backend-resolved original keeps its native actions and any retained grant without immutable facts refuses fallback', () => {
  const f = fixture();
  const { presentationUrl: _url, ...original } = f.item;
  expect(retainedCanvasOutput(original)).toBeUndefined();
  const { artifact: _artifact, ...withoutFacts } = f.item;
  expect(() => retainedCanvasOutput(withoutFacts)).toThrow('immutable artifact');
  f.owner.dispose();
});
