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
/** Real metadata action and service owners prove callback lifetimes through immutable native state, without browser certification. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import {
  prepareLoraMetadata,
  type LoraMetadataOutcome,
  type LoraMetadataResult,
  type LoraResourceAdapter,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { LoraMenuActions } from '../../../../../../frontend/sugarprompt/editor/src/lora-menu-actions.ts';
import type { LoraMetadataHost } from '../../../../../../frontend/sugarprompt/editor/src/lora-action-hosts.ts';
import {
  loraModelTargetAt,
  type PreparedLoraMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-menu-targets.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  prepareLoraPresentation,
  installLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import {
  prepareLoraMetadataActions,
  installLoraMetadataActions,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-metadata-actions.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { hostFixture } from '../transaction/host-document-fixture.ts';

/** Prepare a genuine enabled request, source-current rendered target and validated service replacement. */
function fixture() {
  const native = hostFixture('A\r\n<lora:item:1>');
  const model = {
    promptName: 'item',
    backendValue: 'item.safetensors',
    basename: 'item',
  };
  const observations: LoraMetadataOutcome[] = [];
  const failures: unknown[] = [];
  const accepted: LoraResourceAdapter[] = [];
  let accept: (adapter: LoraResourceAdapter) => void = () => {};
  let observe: () => void = () => {};
  let execute: (signal: AbortSignal) => Promise<LoraMetadataResult> = async () => ({
    status: 'updated',
    catalog: {
      revision: 'next',
      authoritative: true,
      items: [
        {
          model,
          title: 'Item',
          subtitle: null,
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
          modelPageUrl: null,
        },
      ],
    },
    completion: null,
  });
  const envelope: LoraMetadataHost = {
    capability: prepareLoraMetadata({
      revision: 'capability',
      refresh: true,
      thumbnail: false,
      choices: [],
      activeImageId: null,
      execute: (_request, signal) => execute(signal),
    }),
    catalogRevision: 'prior',
    /** Record the validated adapter before allowing synchronous host reentry. */
    accept(adapter) {
      accepted.push(adapter);
      accept(adapter);
    },
    /** Record the typed acknowledgement before testing observer reentry. */
    observe(outcome) {
      observations.push(outcome);
      observe();
    },
    report: (cause) => failures.push(cause),
  };
  native.updates.apply({
    mode: 'preserve',
    configuration: { loraMetadataHost: envelope },
  });
  const resources = readLoraResources(
    [
      {
        name: 'item',
        resolution: 'found',
        model,
        title: 'Item',
        subtitle: null,
        triggerWords: [],
        thumbnail: { status: 'unavailable' },
        modelPageUrl: null,
      },
    ],
    ['item'],
  );
  native.transport.dispatch({
    effects: installLoraPresentation.of(
      prepareLoraPresentation(native.state(), { status: 'ready', resources }),
    ),
  });
  native.transport.dispatch({
    effects: installLoraMetadataActions.of(prepareLoraMetadataActions(native.state())),
  });
  native.transport.dispatch({
    effects: installProjection.of(
      prepareProjection(native.state(), prepareNativeStructure(native.state())),
    ),
  });
  const target = loraModelTargetAt(native.state(), 2)!;
  const item = target.metadata[0]!;
  assert.equal(item.kind, 'action');
  assert.equal(target.metadataLeaves.has(item), true);
  const menu: PreparedLoraMenu = {
    kind: 'inline',
    ...target,
    freshness: native.state().field(editorFreshness),
    pageHost: null,
    metadataHost: envelope,
    items: target.metadata,
    pageItem: null,
    schedule: null,
    editLeaves: new Set(),
  };
  // The existing inert transport proves state/callback ownership only, without implementing browser behavior.
  const owner = new LoraMenuActions(native.transport as unknown as EditorView);
  return {
    native,
    owner,
    envelope,
    observations,
    failures,
    accepted,
    /** Invoke the real action owner with the original prepared menu and leaf. */
    run: () => owner.execute(menu, item),
    /** Install the external synchronous adoption boundary for this specimen. */
    onAccept: (callback: typeof accept) => {
      accept = callback;
    },
    /** Install the external acknowledgement boundary for this specimen. */
    onObserve: (callback: typeof observe) => {
      observe = callback;
    },
    /** Replace only the injected external method, retaining canonical service delegation. */
    onExecute: (callback: typeof execute) => {
      execute = callback;
    },
  };
}

test('accept callback teardown retires follow-up publication', async () => {
  const f = fixture();
  f.onAccept(() => {
    f.owner.dispose();
    f.native.destroy();
  });
  await f.run();
  assert.equal(f.accepted.length, 1);
  assert.deepEqual(f.observations, []);
  assert.deepEqual(f.failures, []);
});

test('a genuinely newer operation inside accept retains sole publication authority', async () => {
  const f = fixture();
  let newer: Promise<void> | undefined;
  f.onAccept(() => {
    if (f.accepted.length === 1) newer = f.run();
  });
  try {
    await f.run();
    await newer;
    assert.equal(f.accepted.length, 2);
    assert.deepEqual(f.observations, [{ status: 'updated', catalogRevision: 'next' }]);
    assert.deepEqual(f.failures, []);
  } finally {
    f.owner.dispose();
  }
});

test('expected accepted catalog adoption preserves the original updated acknowledgement', async () => {
  const f = fixture();
  f.onAccept((adapter) => {
    assert.equal(adapter, f.accepted[0]);
    f.native.updates.apply({
      mode: 'preserve',
      configuration: {
        loraMetadataHost: { ...f.envelope, catalogRevision: adapter.revision },
      },
    });
  });
  try {
    await f.run();
    assert.deepEqual(f.observations, [{ status: 'updated', catalogRevision: 'next' }]);
    assert.deepEqual(f.failures, []);
  } finally {
    f.owner.dispose();
  }
});

test('normal current acceptance publishes once', async () => {
  const f = fixture();
  try {
    await f.run();
    assert.equal(f.accepted.length, 1);
    assert.deepEqual(f.observations, [{ status: 'updated', catalogRevision: 'next' }]);
    assert.deepEqual(f.failures, []);
  } finally {
    f.owner.dispose();
  }
});

test('observer teardown preserves the already delivered updated acknowledgement', async () => {
  const f = fixture();
  f.onObserve(() => f.owner.dispose());
  await f.run();
  assert.equal(f.accepted.length, 1);
  assert.deepEqual(f.observations, [{ status: 'updated', catalogRevision: 'next' }]);
  assert.deepEqual(f.failures, []);
});

test('accept teardown followed by a callback failure cannot report to the retired host', async () => {
  const f = fixture();
  f.onAccept(() => {
    f.owner.dispose();
    throw new Error('Controlled retired callback failure');
  });
  await f.run();
  assert.equal(f.accepted.length, 1);
  assert.deepEqual(f.observations, []);
  assert.deepEqual(f.failures, []);
});

for (const boundary of ['accept', 'observe', 'service'] as const) {
  test(`${boundary} failure remains observable while the operation is current`, async () => {
    const f = fixture();
    const failure = new Error('Controlled callback failure');
    const fail = () => {
      throw failure;
    };
    if (boundary === 'accept') f.onAccept(fail);
    if (boundary === 'observe') f.onObserve(fail);
    if (boundary === 'service') f.onExecute(async () => fail());
    try {
      await f.run();
      assert.deepEqual(f.failures, [failure]);
      assert.equal(f.observations.length, boundary === 'observe' ? 1 : 0);
    } finally {
      f.owner.dispose();
    }
  });
}

test('teardown settles an ignoring host and prevents late service publication', async () => {
  const f = fixture();
  let signal: AbortSignal | undefined;
  f.onExecute((value) => {
    signal = value;
    return new Promise<LoraMetadataResult>(() => {});
  });
  const pending = f.run();
  f.owner.dispose();
  await pending;
  assert.equal(signal?.aborted, true);
  assert.deepEqual(f.accepted, []);
  assert.deepEqual(f.observations, []);
  assert.deepEqual(f.failures, []);
});

test('source retirement during service work admits only the typed cancelled outcome', async () => {
  const f = fixture();
  f.onExecute(async () => {
    f.native.updates.apply({ mode: 'exact-edit', source: 'replacement' });
    return { status: 'unavailable' };
  });
  try {
    await f.run();
    assert.deepEqual(f.accepted, []);
    assert.deepEqual(f.observations, [{ status: 'cancelled' }]);
    assert.deepEqual(f.failures, []);
  } finally {
    f.owner.dispose();
  }
});

test('a current unavailable service reply remains a typed unavailable outcome', async () => {
  const f = fixture();
  f.onExecute(async () => ({ status: 'unavailable' }));
  try {
    await f.run();
    assert.deepEqual(f.accepted, []);
    assert.deepEqual(f.observations, [{ status: 'unavailable' }]);
    assert.deepEqual(f.failures, []);
  } finally {
    f.owner.dispose();
  }
});
