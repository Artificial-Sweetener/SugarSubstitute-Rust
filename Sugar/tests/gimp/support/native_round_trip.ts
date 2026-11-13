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
/** Qualify the common canvas client/server against an actual GIMP-owned Tauri process. */
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { GimpCanvasBridge } from '../../../integrations/gimp/src/infrastructure/GimpCanvasBridge.js';
import { openNativeGimp } from '../../../integrations/gimp/src/infrastructure/TauriNativeGimp.js';

/** Supply independently constructed fixture bytes rather than teaching the proof to reproduce captures. */
export interface NativeRoundTripFixture {
  readonly before: string;
  readonly after: string;
  readonly selection: string;
  readonly png: string;
}

/** Distinguish public-protocol native qualification from the pending ComfyUI execution and GUI lifecycle work. */
export interface NativeRoundTripReport {
  readonly checks: readonly string[];
  readonly documentId: string;
  readonly layerId: string;
}

/** Traverse production CanvasClient, CanvasServer, GIMP translation, Tauri IPC and the native SDK. */
export async function proveNativeRoundTrip(
  host: object,
  fixture: NativeRoundTripFixture,
): Promise<NativeRoundTripReport> {
  const errors: unknown[] = [];
  const checks: string[] = [];
  const native = await openNativeGimp(host, (error) => errors.push(error));
  if (!native) throw new Error('Native qualification requires a GIMP-owned companion process.');
  const session = '00000000-0000-4000-8000-000000000101';
  const bridge = new GimpCanvasBridge(
    session,
    native,
    (message) => {
      client.receive(message);
    },
    (error) => errors.push(error),
  );
  const client = new CanvasClient(session, {
    send: (message) => {
      bridge.receive(message);
    },
    later: (milliseconds, callback) => {
      const timer = setTimeout(callback, milliseconds);
      return () => clearTimeout(timer);
    },
    report: (error) => errors.push(error),
  });
  try {
    const context = await client.readContext();
    const document = context.document;
    if (
      context.canvas.id !== 'gimp' ||
      !context.canvas.capabilities.positionImageLayer ||
      !document
    )
      throw new Error('GIMP native document discovery failed.');
    checks.push('shared_native_context');
    const metadata = await client.sources.execute({ type: 'context' });
    if (metadata.kind !== 'context' || metadata.observesChanges)
      throw new Error('GIMP capture-only metadata was not explicit.');
    checks.push('shared_capture_only_capability');
    const captured = await client.sources.execute({
      type: 'capture',
      document,
      targets: [{ kind: 'composition' }, { kind: 'selection' }],
      maximumSize: 0,
    });
    if (captured.kind !== 'captured') throw new Error('GIMP native source batch failed.');
    const composition = captured.snapshots[0];
    const selection = captured.snapshots[1];
    if (
      composition?.pixels.data !== fixture.before ||
      selection?.pixels.data !== fixture.selection ||
      composition.revision !== selection.revision
    )
      throw new Error('The shared GIMP source batch differs from native fixture pixels.');
    checks.push('shared_coherent_native_rgba_and_selection');
    const request = {
      document,
      image: { format: 'png', encoding: 'base64', data: fixture.png },
      layerName: 'Shared Canvas Result',
      position: { left: -1, top: 2 },
    } as const;
    const stale = await client.insertImage({
      ...request,
      document: { ...document, geometryRevision: 'stale-geometry' },
    });
    if (stale.status !== 'stale') throw new Error('Stale shared GIMP insertion was not refused.');
    checks.push('shared_native_stale_destination_refusal');
    const cancellation = new CanvasCancellation();
    cancellation.cancel();
    const cancelled = await client.insertImage(request, cancellation);
    if (cancelled.status !== 'cancelled')
      throw new Error('Cancelled shared GIMP insertion was not refused.');
    checks.push('shared_native_cancelled_insertion');
    const inserted = await client.insertImage(request);
    if (
      inserted.status !== 'inserted' ||
      inserted.documentId !== document.id ||
      inserted.left !== -1 ||
      inserted.top !== 2 ||
      inserted.width !== 3 ||
      inserted.height !== 2
    )
      throw new Error('Shared GIMP insertion changed its destination or placement.');
    const after = await client.sources.execute({
      type: 'capture',
      document,
      targets: [{ kind: 'composition' }, { kind: 'selection' }],
      maximumSize: 0,
    });
    if (
      after.kind !== 'captured' ||
      after.snapshots[0]?.pixels.data !== fixture.after ||
      after.snapshots[1]?.pixels.data !== fixture.selection
    )
      throw new Error('Native output pixels or the active GIMP selection changed unexpectedly.');
    checks.push('shared_native_exact_output_placement', 'shared_native_selection_preserved');
    const afterMetadata = await client.sources.execute({ type: 'context' });
    if (
      afterMetadata.kind !== 'context' ||
      afterMetadata.layers.length !== metadata.layers.length + 1
    )
      throw new Error('The shared round trip did not create exactly one native layer.');
    checks.push('shared_native_single_layer_commit');
    if (errors.length)
      throw new AggregateError(errors, 'Native shared-canvas qualification reported failures.');
    return { checks, documentId: document.id, layerId: inserted.layerId };
  } finally {
    client.dispose();
    bridge.dispose();
    native.dispose();
  }
}
