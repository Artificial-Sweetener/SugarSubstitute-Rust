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
/** Actual native state proves prepared menu identities without reproducing source parsing or browser placement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  contextMenu,
  installContextMenu,
  type ContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { loraModelTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/lora-menu-targets.ts';
import { prepareTextMenu } from '../../../../../../frontend/sugarprompt/editor/src/text-menu.ts';
import { prepareSavedSegmentMenu } from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import {
  loraPageHost,
  loraMetadataHost,
  type LoraMetadataHost,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-action-hosts.ts';
import { prepareLoraMetadata } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  loraMetadataActions,
  prepareLoraMetadataActions,
  installLoraMetadataActions,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-metadata-actions.ts';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState } from './projection-state.ts';

/** Mount the actual derived owners and a finite host capability, with no native DOM substitute. */
function prepared() {
  const capability = new Compartment();
  const host = Object.freeze({
    adapter: { open: async () => ({ status: 'opened' as const }) },
    observe: () => {},
    report: () => {},
  });
  let state = projectionState('🙂\r\n<lora:alias:0.80>, tail');
  state = state.update({
    effects: StateEffect.appendConfig.of([contextMenu, capability.of(loraPageHost.of(host))]),
  }).state;
  const metadata = prepareLoraPresentation(state, {
    status: 'ready',
    resources: readLoraResources(
      [
        {
          name: 'alias',
          resolution: 'found',
          model: {
            promptName: 'folder/real',
            backendValue: 'folder/real.safetensors',
            basename: 'real',
          },
          modelPageUrl: 'https://models.invalid/1',
          title: 'display',
          subtitle: null,
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
        },
      ],
      ['alias'],
    ),
  });
  state = state.update({ effects: installLoraPresentation.of(metadata) }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  const target = loraModelTargetAt(state, 3)!;
  const pageItem = {
    kind: 'page' as const,
    label: target.page!.label,
    request: target.page!,
  };
  const menu: ContextMenu = {
    surface: 'local',
    utilities: null,
    scheduleItem: null,
    presets: prepareSavedSegmentMenu(state, 3),
    text: prepareTextMenu(state, { readable: false, writable: false }),
    kind: 'inline',
    from: target.from,
    to: target.to,
    diagnostic: null,
    queue: null,
    queueItem: null,
    renderingItem: {
      kind: 'rich-rendering',
      label: 'Rich prompt rendering',
      checked: true,
    },
    actions: {
      kind: 'inline',
      ...target,
      freshness: state.field(editorFreshness),
      pageHost: host,
      metadataHost: null,
      pageItem,
      items: [pageItem],
      schedule: null,
      editLeaves: new Set(),
    },
    items: [pageItem],
    anchor: { left: 1, right: 1, top: 1, bottom: 1 },
  };
  return { state, metadata, menu, capability };
}

test('logical chip boundaries reuse actual prepared page objects through selection and leave source/history untouched', () => {
  const fixture = prepared();
  const { metadata, menu } = fixture;
  let { state } = fixture;
  assert.equal(menu.from, 3);
  assert.equal(menu.to, 20);
  assert.equal(menu.actions?.kind === 'inline' && menu.actions.page, metadata.pages.get('alias'));
  state = state.update({ selection: { anchor: state.doc.length } }).state;
  assert.equal(state.field(loraPresentation).pages, metadata.pages);
  assert.equal(
    loraModelTargetAt(state, menu.from)?.page,
    menu.actions?.kind === 'inline' && menu.actions.page,
  );
  state = state.update({ effects: installContextMenu.of(menu) }).state;
  assert.equal(state.field(contextMenu), null);
  const current = {
    ...menu,
    text: prepareTextMenu(state, { readable: false, writable: false }),
  };
  state = state.update({ effects: installContextMenu.of(current) }).state;
  assert.equal(state.field(contextMenu), current);
  assert.equal(undoDepth(state), 0);
  assert.equal(state.doc.toString() === '🙂\n<lora:alias:0.80>, tail', true);
});

test('source, command context, metadata, capability and selection retirement reject obsolete menu publication', () => {
  for (const kind of ['source', 'context', 'metadata', 'host', 'selection'] as const) {
    const fixture = prepared();
    const { menu, capability } = fixture;
    let { state } = fixture;
    state = state.update({ effects: installContextMenu.of(menu) }).state;
    switch (kind) {
      case 'source':
        state = state.update(exactReplacement(state, 0, 2, 'cat')).state;
        break;
      case 'context':
        state = state.update({
          effects: invalidateCommandContext.of(null),
        }).state;
        break;
      case 'metadata':
        state = state.update({
          effects: installLoraPresentation.of(
            prepareLoraPresentation(state, {
              status: 'ready',
              resources: [{ name: 'alias', resolution: 'missing' }],
            }),
          ),
        }).state;
        break;
      case 'host':
        state = state.update({
          effects: capability.reconfigure(loraPageHost.of(null)),
        }).state;
        break;
      case 'selection':
        state = state.update({ selection: { anchor: state.doc.length } }).state;
        break;
    }
    assert.equal(state.field(contextMenu), null);
    state = state.update({ effects: installContextMenu.of(menu) }).state;
    assert.equal(state.field(contextMenu), null);
  }
});

test('actual metadata trees and enabled leaf sets are reused through selection and rejected after capability or resource replacement', () => {
  let { state } = prepared();
  const configuration = new Compartment();
  const host: LoraMetadataHost = {
    capability: prepareLoraMetadata({
      revision: 'actions',
      refresh: true,
      thumbnail: true,
      choices: [],
      activeImageId: null,
      execute: async () => ({ status: 'unavailable' }),
    }),
    catalogRevision: 'catalog',
    accept: () => {},
    observe: () => {},
    report: () => {},
  };
  state = state.update({
    effects: StateEffect.appendConfig.of([
      loraMetadataActions,
      configuration.of(loraMetadataHost.of(host)),
    ]),
  }).state;
  const actions = prepareLoraMetadataActions(state);
  state = state.update({
    effects: installLoraMetadataActions.of(actions),
  }).state;
  const target = loraModelTargetAt(state, 3)!;
  const tree = actions.byName.get('alias')!;
  assert.equal(target.metadata, tree.items);
  assert.equal(target.metadataLeaves, tree.leaves);
  assert.equal(tree.leaves.size, 1);
  assert.equal(tree.leaves.has(tree.items[0]!), true);
  assert.equal(tree.leaves.has(tree.items[1]!), false);
  state = state.update({ selection: { anchor: state.doc.length } }).state;
  assert.equal(state.field(loraMetadataActions), actions);
  assert.equal(loraModelTargetAt(state, 3)?.metadata, tree.items);
  assert.equal(undoDepth(state), 0);
  state = state.update({
    effects: configuration.reconfigure(loraMetadataHost.of({ ...host })),
  }).state;
  assert.equal(loraModelTargetAt(state, 3)?.metadata.length, 0);
  state = state.update({
    effects: installLoraMetadataActions.of(actions),
  }).state;
  assert.equal(loraModelTargetAt(state, 3)?.metadata.length, 0);
  const fresh = prepareLoraMetadataActions(state);
  state = state.update({ effects: installLoraMetadataActions.of(fresh) }).state;
  assert.notEqual(loraModelTargetAt(state, 3)?.metadata, tree.items);
  state = state.update({
    effects: installLoraPresentation.of(
      prepareLoraPresentation(state, {
        status: 'ready',
        resources: [{ name: 'alias', resolution: 'missing' }],
      }),
    ),
  }).state;
  state = state.update({ effects: installLoraMetadataActions.of(fresh) }).state;
  assert.equal(loraModelTargetAt(state, 3), null);
});
