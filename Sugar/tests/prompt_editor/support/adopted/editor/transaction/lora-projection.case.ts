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
/** Real native state proves LoRA preparation, source coordinates and metadata freshness independently of browser rendering. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import type { EditorState } from '@codemirror/state';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  loraMedia,
  installLoraMedia,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import type { LoraMediaSnapshot } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
  projectedLora,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState, projectionRanges } from './projection-state.ts';

/** Observe the actual compiled widgets rather than reconstructing the owner's rendering rules. */
function widgets(state: EditorState): readonly LoraWidget[] {
  const result: LoraWidget[] = [];
  state.field(richProjection).all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof LoraWidget) result.push(value.spec.widget);
  });
  return result;
}

/** Publish through the real owner, using its shared semantic cache. */
function prepare(state: EditorState): EditorState {
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

test('LoRA projection preserves Unicode, CRLF, secondary weights and block source with one outer atomic range', () => {
  const source = '🙂\r\n<lora:folder/model: +0.80:0.60:block>, <lora:bad:abc>';
  const state = prepare(projectionState(source));
  assert.deepEqual(projectionRanges(state), [[3, source.indexOf('>')]]);
  assert.equal(widgets(state).length, 1);
  assert.equal(widgets(state)[0]?.resource.name, 'folder/model');
  assert.equal(widgets(state)[0]?.titleLabel, 'model');
  assert.equal(widgets(state)[0]?.weight, '+0.80');
  assert.equal(widgets(state)[0]?.resource.resolution, 'unavailable');
  assert.equal(state.doc.toString() === source.replace('\r\n', '\n'), true);
  assert.equal(undoDepth(state), 0);
});

test('empty metadata keeps actual non-LoRA decorations ready while empty authority does not follow source edits', () => {
  let state = prepare(projectionState('(cat:1.20)'));
  const original = state.field(richProjection),
    ready = prepareLoraPresentation(state, { status: 'ready', resources: [] });
  state = state.update({ effects: installLoraPresentation.of(ready) }).state;
  const rebound = state.field(richProjection);
  assert.strictEqual(rebound.all, original.all);
  assert.strictEqual(rebound.tokens, original.tokens);
  assert.strictEqual(rebound.sourceIdentity, original.sourceIdentity);
  assert.strictEqual(rebound.loraPresentation, ready);
  state = state.update(exactReplacement(state, 10, 10, ', <lora:real:1>')).state;
  assert.notStrictEqual(
    state.field(loraPresentation).sourceIdentity,
    state.field(editorFreshness).source,
  );
  assert.equal(state.field(richProjection).sourceIdentity, null);
});

test('validated metadata reuses real records and prepared ranges through selection; source changes reject stale publication', () => {
  let state = projectionState('<lora:model:0.80>, tail');
  const resources = readLoraResources(
    [
      {
        name: 'model',
        resolution: 'found',
        model: {
          promptName: 'folder/real',
          backendValue: 'folder/real.safetensors',
          basename: 'real',
        },
        modelPageUrl: null,
        title: '<literal>',
        subtitle: 'version',
        triggerWords: [],
        thumbnail: { status: 'unavailable' },
      },
    ],
    ['model'],
  );
  const metadata = prepareLoraPresentation(state, {
    status: 'ready',
    resources,
  });
  state = state.update({ effects: installLoraPresentation.of(metadata) }).state;
  state = prepare(state);
  const projection = state.field(richProjection);
  assert.equal(widgets(state)[0]?.resource, resources[0]);
  state = state.update({ selection: { anchor: state.doc.length } }).state;
  assert.equal(state.field(loraPresentation), metadata);
  assert.equal(state.field(richProjection), projection);
  state = state.update(exactReplacement(state, 6, 11, 'other')).state;
  state = state.update({
    effects: [installLoraPresentation.of(metadata), installProjection.of(projection)],
  }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  assert.strictEqual(state.field(loraPresentation), metadata);
  assert.strictEqual(projectedLora(state, 'model'), resources[0]);
  assert.equal(projectedLora(state, 'other').resolution, 'pending-authority');
  state = prepare(state);
  assert.equal(widgets(state)[0]?.resource.name, 'other');
  assert.equal(widgets(state)[0]?.resource.resolution, 'pending-authority');
});

test('catalog replacement rejects projection from a previous metadata identity without changing source/history', () => {
  let state = prepare(projectionState('<lora:model:0.80>'));
  const previous = state.field(richProjection);
  const replacement = prepareLoraPresentation(state, {
    status: 'ready',
    resources: [{ name: 'model', resolution: 'ambiguous', candidateCount: null }],
  });
  state = state.update({
    effects: installLoraPresentation.of(replacement),
  }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(previous) }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = prepare(state);
  assert.equal(widgets(state)[0]?.status, 'Ambiguous model name');
  assert.equal(state.doc.toString(), '<lora:model:0.80>');
  assert.equal(undoDepth(state), 0);
});

test('catalog-stable aliases survive numeric source edits while transient authority and stale publications remain source-bound', () => {
  let state = projectionState('<lora:model:1.05>, <lora:waiting:0.50>');
  const metadata = prepareLoraPresentation(state, {
    status: 'ready',
    resources: [
      { name: 'model', resolution: 'missing' },
      { name: 'waiting', resolution: 'pending-authority' },
    ],
  });
  state = state.update({ effects: installLoraPresentation.of(metadata) }).state;
  state = state.update(exactReplacement(state, 12, 16, '1.00')).state;
  assert.strictEqual(projectedLora(state, 'model'), metadata.byName.get('model'));
  assert.notStrictEqual(projectedLora(state, 'waiting'), metadata.byName.get('waiting'));
  const retired = prepareLoraPresentation(state, {
    status: 'ready',
    resources: [{ name: 'new', resolution: 'missing' }],
  });
  state = state.update(exactReplacement(state, 6, 11, 'other')).state;
  state = state.update({ effects: installLoraPresentation.of(retired) }).state;
  assert.strictEqual(state.field(loraPresentation), metadata);
  state = prepare(state);
  assert.equal(widgets(state)[0]?.resource.name, 'other');
  assert.equal(widgets(state)[0]?.resource.resolution, 'pending-authority');
  assert.equal(projectedLora(state, 'new').resolution, 'pending-authority');
});

test('an empty metadata index follows source changes without replacing its catalog snapshot or map', () => {
  let state = projectionState('(cat:1.20)');
  const original = state.field(loraPresentation);
  state = state.update(exactReplacement(state, 1, 4, 'dog')).state;
  const current = state.field(loraPresentation);
  assert.equal(current.snapshot, original.snapshot);
  assert.equal(current.byName, original.byName);
  assert.notEqual(current.sourceIdentity, original.sourceIdentity);
});

test('shared raster publications retain actual prepared input, retire old projections and reject source/context supersession', () => {
  let state = prepare(projectionState('<lora:model:0.80>'));
  const before = state.field(richProjection);
  const reference = {
    status: 'ready',
    url: 'https://media.invalid/image',
    width: 16,
    height: 24,
  } as const;
  const media: LoraMediaSnapshot = new Map([[reference, { status: 'pending' }]]);
  const publication = { freshness: state.field(editorFreshness), media };
  const metadata = state.field(loraPresentation);
  state = state.update({ effects: installLoraMedia.of(publication) }).state;
  assert.equal(state.field(loraMedia), media);
  assert.equal(state.field(loraPresentation), metadata);
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(before) }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = prepare(state);
  assert.equal(state.field(richProjection).loraMedia, media);
  const projection = state.field(richProjection);
  state = state.update({ selection: { anchor: state.doc.length } }).state;
  assert.equal(state.field(richProjection), projection);
  const rejected: LoraMediaSnapshot = new Map();
  state = state.update({
    effects: [
      invalidateCommandContext.of(null),
      installLoraMedia.of({ ...publication, media: rejected }),
    ],
  }).state;
  assert.equal(state.field(loraMedia), media);
  state = state.update(exactReplacement(state, 6, 11, 'other')).state;
  state = state.update({
    effects: installLoraMedia.of({ ...publication, media: rejected }),
  }).state;
  assert.equal(state.field(loraMedia), media);
});
