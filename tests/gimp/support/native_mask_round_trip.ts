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
/** Qualify the existing mask client/server against an actual GIMP-owned native companion. */
import type { CanvasMaskReference } from '../../../integrations/contracts/CanvasMasksProtocol.generated.js';
import type { GimpNativePort } from '../../../integrations/gimp/src/application/NativeGimpSession.js';
import { GimpCanvasMasks } from '../../../integrations/gimp/src/infrastructure/GimpCanvasMasks.js';
import { openNativeGimp } from '../../../integrations/gimp/src/infrastructure/TauriNativeGimp.js';
import { CanvasMaskClient } from '../../../integrations/shared/CanvasMaskClient.js';
import { CanvasMaskServer } from '../../../integrations/shared/CanvasMaskServer.js';
import {
  encodeMaskPixels,
  decodeMaskPixels,
} from '../../../integrations/shared/CanvasMaskPixels.js';

/** Keep each public transport lifetime independent of the native viewer connection. */
function publicMasks(native: GimpNativePort, session: string, errors: unknown[]) {
  const host = new GimpCanvasMasks(native, (error) => errors.push(error));
  const server = new CanvasMaskServer(session, {
    changes: host.changes,
    execute: (command) => host.execute(command),
    send: (value) => {
      client.receive(value);
    },
    report: (error) => errors.push(error),
  });
  const client = new CanvasMaskClient(
    session,
    {
      send: (value) => {
        server.receive(value);
      },
      later: (milliseconds, callback) => {
        const timer = setTimeout(callback, milliseconds);
        return () => clearTimeout(timer);
      },
      report: (error) => errors.push(error),
    },
    () => 'gimp',
  );
  return {
    client,
    dispose: () => {
      client.dispose();
      server.dispose();
      host.dispose();
    },
  };
}

/** Traverse production public validation, GIMP mask translation, Tauri IPC and native coverage storage. */
export async function proveNativeMaskRoundTrip(
  host: object,
  expectedSelection: string,
): Promise<{
  readonly checks: readonly string[];
  readonly masks: readonly CanvasMaskReference[];
}> {
  const errors: unknown[] = [];
  const native = await openNativeGimp(host, (error) => errors.push(error));
  if (!native) throw new Error('Native mask qualification requires a GIMP-owned companion.');
  const first = publicMasks(native, '00000000-0000-4000-8000-000000000201', errors);
  let next: ReturnType<typeof publicMasks> | null = null;
  const checks: string[] = [];
  try {
    const context = await first.client.execute({ type: 'context' });
    if (
      context.kind !== 'context' ||
      !context.supported ||
      context.observesChanges ||
      !context.document
    )
      throw new Error('GIMP did not negotiate the capture-only authored mask capability.');
    const document = context.document;
    const observation = first.client.changes.subscribe(() => {
      throw new Error('Unexpected native mask observation.');
    });
    if ((await observation.ready) !== 'unsupported')
      throw new Error('Unavailable native observation was not explicit.');
    observation.stop();
    checks.push('shared_mask_capability_and_observation_negotiation');
    const pattern = encodeMaskPixels(
      document.width,
      document.height,
      Uint8Array.from({ length: document.width * document.height }, (_, index) => index % 256),
    );
    const blank = encodeMaskPixels(
      document.width,
      document.height,
      new Uint8Array(document.width * document.height),
    );
    const masks: CanvasMaskReference[] = [];
    for (const source of ['selection', 'blank', 'pixels'] as const) {
      const result = await first.client.execute({
        type: 'create',
        document,
        name: `Native ${source} proof`,
        source,
        pixels: source === 'pixels' ? pattern : null,
      });
      if (result.kind !== 'created') throw new Error(`The native ${source} mask was not created.`);
      masks.push(result.mask);
    }
    checks.push('shared_native_mask_creation_selection_blank_and_pixels');
    const active = masks[2];
    if (!active) throw new Error('Missing native imported mask identity.');
    const activated = await first.client.execute({ type: 'activate', mask: active });
    if (activated.kind !== 'activated') throw new Error('Native mask editing did not activate.');
    const captured = await first.client.execute({ type: 'capture', masks, maximumSize: 0 });
    if (
      captured.kind !== 'captured' ||
      captured.snapshots.length !== 3 ||
      captured.snapshots[0]?.pixels.data !== expectedSelection ||
      captured.snapshots[1]?.pixels.data !== blank.data ||
      captured.snapshots[2]?.pixels.data !== pattern.data ||
      new Set(captured.snapshots.map((snapshot) => snapshot.revision)).size !== 1
    )
      throw new Error(
        'The native authored mask batch differs from the independent expected coverage.',
      );
    for (const snapshot of captured.snapshots) decodeMaskPixels(snapshot.pixels);
    checks.push('shared_coherent_native_mask_coverage', 'shared_all_coverage_values_preserved');
    const stale = await first.client.execute({
      type: 'activate',
      mask: {
        ...active,
        document: { ...document, geometryRevision: 'stale-geometry' },
      },
    });
    if (stale.kind !== 'failure' || stale.status !== 'stale')
      throw new Error('A stale native mask binding was not refused.');
    checks.push('shared_native_stale_mask_refusal');
    if ((await first.client.execute({ type: 'release', mask: active })).kind !== 'activated')
      throw new Error('Native mask editing did not release.');
    first.dispose();
    next = publicMasks(native, '00000000-0000-4000-8000-000000000202', errors);
    const restored = await next.client.execute({ type: 'capture', masks, maximumSize: 0 });
    if (restored.kind !== 'captured' || restored.snapshots[2]?.pixels.data !== pattern.data)
      throw new Error('The saved native mask association did not survive adapter replacement.');
    checks.push('shared_adapter_replacement_retains_native_mask_associations');
    if (errors.length)
      throw new AggregateError(errors, 'Native mask qualification reported errors.');
    return { checks, masks };
  } finally {
    first.dispose();
    next?.dispose();
    native.dispose();
  }
}
