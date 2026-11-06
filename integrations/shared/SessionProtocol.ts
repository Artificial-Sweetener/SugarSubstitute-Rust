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
/** Validate protocol correlation and admission independently of transport and lifecycle state. */
import type {
  SessionBinding,
  SessionCapabilities,
  SessionFailure,
  SessionFence,
  SessionMessage,
  SessionScope,
} from '../contracts/SessionProtocol.generated.js';
import { isSessionMessage } from './SessionValidation.generated.js';
import {
  sessionDigest,
  sessionFile,
  sessionJsonScalars,
  sessionScope,
  sessionUuid,
  utf8Bytes,
} from './SessionPrimitives.js';
import { sessionOutput } from './SessionSnapshot.js';

/** Keep actionable machine-readable errors separate from localized product copy. */
export type SessionErrorCode = SessionFailure['code'];
/** Maximum encoded message bytes; checkpoint transfers have their own 128 MiB admission. */
export const SESSION_MESSAGE_BYTES = 524288;
/** Maximum raw UTF-8 chunk bytes, before JSON escaping. */
export const SESSION_CHUNK_BYTES = 65536;

/** Validate unique durable-to-fresh-runtime bindings without retaining live runtime identities. */
function bindings(values: readonly SessionBinding[]): boolean {
  return (
    new Set(values.map((item) => item.entryId)).size === values.length &&
    new Set(values.map((item) => item.runtimeSession)).size === values.length &&
    values.every((item) => sessionUuid(item.entryId) && item.runtimeSession.length > 0)
  );
}

/** Require the complete revision set; tab reordering does not change binding equality. */
export function sameSessionFence(left: SessionFence, right: SessionFence): boolean {
  if (
    left.webview !== right.webview ||
    left.revision !== right.revision ||
    left.bindings.length !== right.bindings.length ||
    !bindings(left.bindings) ||
    !bindings(right.bindings)
  )
    return false;
  return left.bindings.every((item) =>
    right.bindings.some(
      (other) =>
        item.entryId === other.entryId &&
        item.runtimeSession === other.runtimeSession &&
        item.generation === other.generation &&
        item.graphRevision === other.graphRevision &&
        item.sourceRevision === other.sourceRevision &&
        item.navigationRevision === other.navigationRevision,
    ),
  );
}

/** Admit protocol values only after comparison with independently authorized scope and origin. */
export function sessionMessageError(
  value: unknown,
  scope: SessionScope,
  origin: string,
): SessionErrorCode | null {
  if (
    !isSessionMessage(value) ||
    !sessionJsonScalars(value) ||
    !sessionScope(value.scope) ||
    !sessionUuid(value.requestId)
  )
    return 'invalid';
  if (!origin || value.origin !== origin) return 'untrusted-origin';
  if (
    Object.keys(scope).some(
      (key) => scope[key as keyof SessionScope] !== value.scope[key as keyof SessionScope],
    )
  )
    return 'stale';
  const operation = value.operation;
  switch (operation.kind) {
    case 'ready':
      if (!sessionUuid(operation.fence.webview) || !bindings(operation.fence.bindings))
        return 'invalid';
      if (
        (operation.capabilities.capture || operation.capabilities.materialize) &&
        !operation.capabilities.chunkTransfer
      )
        return 'invalid';
      break;
    case 'capture':
    case 'materialize':
      if (
        !sessionUuid(operation.fence.webview) ||
        !bindings(operation.fence.bindings) ||
        !sessionUuid(operation.kind === 'capture' ? operation.checkpointId : operation.transferId)
      )
        return 'invalid';
      break;
    case 'begin': {
      const transfer = operation.transfer;
      if (
        ![transfer.id, transfer.checkpointId].every(sessionUuid) ||
        !sessionDigest(transfer.digest) ||
        transfer.bytes < transfer.chunks ||
        transfer.bytes > transfer.chunks * SESSION_CHUNK_BYTES
      )
        return 'invalid';
      break;
    }
    case 'chunk':
      if (
        !sessionUuid(operation.transferId) ||
        !operation.text ||
        utf8Bytes(operation.text) > SESSION_CHUNK_BYTES
      )
        return 'invalid';
      break;
    case 'commit':
      if (!sessionUuid(operation.transferId)) return 'invalid';
      break;
    case 'cancel':
      if (!sessionUuid(operation.targetRequestId)) return 'invalid';
      break;
    case 'acknowledged':
      if (!sessionUuid(operation.checkpointId) || !bindings(operation.bindings)) return 'invalid';
      break;
    case 'error':
      if (
        [operation.failure.entryId, operation.failure.outputId].some(
          (id) => id !== null && !sessionUuid(id),
        )
      )
        return 'invalid';
      break;
    case 'resolve':
      if (!sessionOutput(operation.output) || operation.output.backend !== scope.backend)
        return 'invalid';
      break;
    case 'resolved':
      if (
        !sessionUuid(operation.outputId) ||
        !sessionDigest(operation.digest) ||
        !sessionFile(operation.file)
      )
        return 'invalid';
      break;
  }
  return utf8Bytes(JSON.stringify(value)) <= SESSION_MESSAGE_BYTES ? null : 'too-large';
}

/** Reject unsupported operations, opt-out, loading state and stale generations before effects. */
export function sessionAdmission(
  message: SessionMessage,
  capabilities: SessionCapabilities,
  committed: boolean,
  workflowPersistence: boolean,
  current: SessionFence,
): SessionErrorCode | null {
  const operation = message.operation;
  if (operation.kind === 'ready') {
    const fence = operation.fence;
    if (fence.webview !== current.webview || fence.revision < current.revision) return 'stale';
    if (
      fence.revision === current.revision &&
      (!sameSessionFence(fence, current) ||
        operation.committed !== committed ||
        operation.workflowPersistence !== workflowPersistence ||
        Object.keys(capabilities).some(
          (key) =>
            capabilities[key as keyof SessionCapabilities] !==
            operation.capabilities[key as keyof SessionCapabilities],
        ))
    )
      return 'stale';
    return null;
  }
  if (operation.kind === 'begin' || operation.kind === 'chunk' || operation.kind === 'commit')
    return capabilities.chunkTransfer ? null : 'unsupported';
  if (operation.kind === 'resolve') return capabilities.artifactResolution ? null : 'unsupported';
  if (operation.kind !== 'capture' && operation.kind !== 'materialize') return null;
  if (!capabilities.chunkTransfer || !capabilities[operation.kind]) return 'unsupported';
  if (!workflowPersistence) return 'opted-out';
  if (!committed) return 'not-committed';
  return sameSessionFence(operation.fence, current) ? null : 'stale';
}
