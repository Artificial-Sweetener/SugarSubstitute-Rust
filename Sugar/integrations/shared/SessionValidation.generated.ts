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
/** Generated protocol validation; edit the authoritative session.schema.json. */
import type * as Contract from '../contracts/SessionProtocol.generated.js';
import {
  isSessionOutput,
  isSessionFile,
  isSessionScope,
} from './SessionOutputValidation.generated.js';
import { isSessionCheckpoint } from './SessionSnapshotValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded SessionBinding payload before dispatch. */
export function isSessionBinding(value: unknown): value is Contract.SessionBinding {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'entryId') &&
    typeof value['entryId'] === 'string' &&
    value['entryId'].length <= 36 &&
    Object.hasOwn(value, 'runtimeSession') &&
    typeof value['runtimeSession'] === 'string' &&
    value['runtimeSession'].length <= 256 &&
    Object.hasOwn(value, 'generation') &&
    typeof value['generation'] === 'number' &&
    Number.isSafeInteger(value['generation']) &&
    value['generation'] >= 0 &&
    value['generation'] <= 9007199254740991 &&
    Object.hasOwn(value, 'graphRevision') &&
    typeof value['graphRevision'] === 'number' &&
    Number.isSafeInteger(value['graphRevision']) &&
    value['graphRevision'] >= 0 &&
    value['graphRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'sourceRevision') &&
    typeof value['sourceRevision'] === 'number' &&
    Number.isSafeInteger(value['sourceRevision']) &&
    value['sourceRevision'] >= 0 &&
    value['sourceRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'navigationRevision') &&
    typeof value['navigationRevision'] === 'number' &&
    Number.isSafeInteger(value['navigationRevision']) &&
    value['navigationRevision'] >= 0 &&
    value['navigationRevision'] <= 9007199254740991
  );
}
/** Validate a bounded SessionFence payload before dispatch. */
export function isSessionFence(value: unknown): value is Contract.SessionFence {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'webview') &&
    typeof value['webview'] === 'string' &&
    value['webview'].length <= 36 &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'bindings') &&
    Array.isArray(value['bindings']) &&
    value['bindings'].length <= 256 &&
    value['bindings'].every((item: unknown) => isSessionBinding(item))
  );
}
/** Validate a bounded SessionCapabilities payload before dispatch. */
export function isSessionCapabilities(value: unknown): value is Contract.SessionCapabilities {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'capture') &&
    typeof value['capture'] === 'boolean' &&
    Object.hasOwn(value, 'materialize') &&
    typeof value['materialize'] === 'boolean' &&
    Object.hasOwn(value, 'chunkTransfer') &&
    typeof value['chunkTransfer'] === 'boolean' &&
    Object.hasOwn(value, 'artifactResolution') &&
    typeof value['artifactResolution'] === 'boolean'
  );
}
/** Validate a bounded SessionTransfer payload before dispatch. */
export function isSessionTransfer(value: unknown): value is Contract.SessionTransfer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 36 &&
    Object.hasOwn(value, 'checkpointId') &&
    typeof value['checkpointId'] === 'string' &&
    value['checkpointId'].length <= 36 &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'bytes') &&
    typeof value['bytes'] === 'number' &&
    Number.isSafeInteger(value['bytes']) &&
    value['bytes'] >= 1 &&
    value['bytes'] <= 134217728 &&
    Object.hasOwn(value, 'chunks') &&
    typeof value['chunks'] === 'number' &&
    Number.isSafeInteger(value['chunks']) &&
    value['chunks'] >= 1 &&
    value['chunks'] <= 8192 &&
    Object.hasOwn(value, 'digest') &&
    typeof value['digest'] === 'string' &&
    value['digest'].length <= 64 &&
    Object.hasOwn(value, 'direction') &&
    (value['direction'] === 'capture' || value['direction'] === 'materialize')
  );
}
/** Validate a bounded SessionFailure payload before dispatch. */
export function isSessionFailure(value: unknown): value is Contract.SessionFailure {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'code') &&
    (value['code'] === 'unsupported' ||
      value['code'] === 'invalid' ||
      value['code'] === 'too-large' ||
      value['code'] === 'stale' ||
      value['code'] === 'cancelled' ||
      value['code'] === 'untrusted-origin' ||
      value['code'] === 'unavailable' ||
      value['code'] === 'identity-mismatch' ||
      value['code'] === 'quota' ||
      value['code'] === 'storage' ||
      value['code'] === 'migration-required' ||
      value['code'] === 'opted-out' ||
      value['code'] === 'not-committed') &&
    Object.hasOwn(value, 'retryable') &&
    typeof value['retryable'] === 'boolean' &&
    Object.hasOwn(value, 'entryId') &&
    ((typeof value['entryId'] === 'string' && value['entryId'].length <= 36) ||
      value['entryId'] === null) &&
    Object.hasOwn(value, 'outputId') &&
    ((typeof value['outputId'] === 'string' && value['outputId'].length <= 36) ||
      value['outputId'] === null)
  );
}
/** Validate a bounded SessionOperation payload before dispatch. */
export function isSessionOperation(value: unknown): value is Contract.SessionOperation {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'ready' &&
      Object.hasOwn(value, 'capabilities') &&
      isSessionCapabilities(value['capabilities']) &&
      Object.hasOwn(value, 'committed') &&
      typeof value['committed'] === 'boolean' &&
      Object.hasOwn(value, 'workflowPersistence') &&
      typeof value['workflowPersistence'] === 'boolean' &&
      Object.hasOwn(value, 'fence') &&
      isSessionFence(value['fence'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'capture' &&
      Object.hasOwn(value, 'fence') &&
      isSessionFence(value['fence']) &&
      Object.hasOwn(value, 'checkpointId') &&
      typeof value['checkpointId'] === 'string' &&
      value['checkpointId'].length <= 36 &&
      Object.hasOwn(value, 'revision') &&
      typeof value['revision'] === 'number' &&
      Number.isSafeInteger(value['revision']) &&
      value['revision'] >= 0 &&
      value['revision'] <= 9007199254740991) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'materialize' &&
      Object.hasOwn(value, 'fence') &&
      isSessionFence(value['fence']) &&
      Object.hasOwn(value, 'transferId') &&
      typeof value['transferId'] === 'string' &&
      value['transferId'].length <= 36) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'begin' &&
      Object.hasOwn(value, 'transfer') &&
      isSessionTransfer(value['transfer'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'chunk' &&
      Object.hasOwn(value, 'transferId') &&
      typeof value['transferId'] === 'string' &&
      value['transferId'].length <= 36 &&
      Object.hasOwn(value, 'index') &&
      typeof value['index'] === 'number' &&
      Number.isSafeInteger(value['index']) &&
      value['index'] >= 0 &&
      value['index'] <= 8191 &&
      Object.hasOwn(value, 'text') &&
      typeof value['text'] === 'string' &&
      value['text'].length <= 65536) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'commit' &&
      Object.hasOwn(value, 'transferId') &&
      typeof value['transferId'] === 'string' &&
      value['transferId'].length <= 36) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'cancel' &&
      Object.hasOwn(value, 'targetRequestId') &&
      typeof value['targetRequestId'] === 'string' &&
      value['targetRequestId'].length <= 36) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'acknowledged' &&
      Object.hasOwn(value, 'checkpointId') &&
      typeof value['checkpointId'] === 'string' &&
      value['checkpointId'].length <= 36 &&
      Object.hasOwn(value, 'revision') &&
      typeof value['revision'] === 'number' &&
      Number.isSafeInteger(value['revision']) &&
      value['revision'] >= 0 &&
      value['revision'] <= 9007199254740991 &&
      Object.hasOwn(value, 'phase') &&
      (value['phase'] === 'captured' ||
        value['phase'] === 'materialized' ||
        value['phase'] === 'durable' ||
        value['phase'] === 'restored' ||
        value['phase'] === 'cancelled') &&
      Object.hasOwn(value, 'bindings') &&
      Array.isArray(value['bindings']) &&
      value['bindings'].length <= 256 &&
      value['bindings'].every((item: unknown) => isSessionBinding(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'error' &&
      Object.hasOwn(value, 'failure') &&
      isSessionFailure(value['failure'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'resolve' &&
      Object.hasOwn(value, 'output') &&
      isSessionOutput(value['output'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'resolved' &&
      Object.hasOwn(value, 'outputId') &&
      typeof value['outputId'] === 'string' &&
      value['outputId'].length <= 36 &&
      Object.hasOwn(value, 'digest') &&
      typeof value['digest'] === 'string' &&
      value['digest'].length <= 64 &&
      Object.hasOwn(value, 'bytes') &&
      typeof value['bytes'] === 'number' &&
      Number.isSafeInteger(value['bytes']) &&
      value['bytes'] >= 1 &&
      value['bytes'] <= 8589934592 &&
      Object.hasOwn(value, 'file') &&
      isSessionFile(value['file']))
  );
}
/** Validate a bounded SessionMessage payload before dispatch. */
export function isSessionMessage(value: unknown): value is Contract.SessionMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.desktop-session' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'string' &&
    value['requestId'].length <= 36 &&
    Object.hasOwn(value, 'scope') &&
    isSessionScope(value['scope']) &&
    Object.hasOwn(value, 'origin') &&
    typeof value['origin'] === 'string' &&
    value['origin'].length <= 2048 &&
    Object.hasOwn(value, 'operation') &&
    isSessionOperation(value['operation'])
  );
}
/** Validate a bounded SessionValue payload before dispatch. */
export function isSessionValue(value: unknown): value is Contract.SessionValue {
  return isSessionCheckpoint(value) || isSessionMessage(value);
}
export {
  isSessionScope,
  isSessionSource,
  isSessionScene,
  isSessionFile,
  isSessionAsset,
  isSessionOutput,
  isSessionSubmission,
} from './SessionOutputValidation.generated.js';
export {
  isSessionSelection,
  isSessionCamera,
  isSessionInspection,
  isSessionComparison,
  isSessionNavigation,
  isSessionView,
  isSessionGraphView,
} from './SessionViewValidation.generated.js';
export {
  isSessionDocument,
  isSessionInput,
  isSessionEntry,
  isSessionRect,
  isSessionPlacement,
  isSessionCheckpoint,
} from './SessionSnapshotValidation.generated.js';
