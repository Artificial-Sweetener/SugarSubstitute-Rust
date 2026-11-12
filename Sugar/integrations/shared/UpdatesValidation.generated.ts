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
/** Generated protocol validation; edit the authoritative updates.schema.json. */
import type * as Contract from '../contracts/UpdatesProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded UpdatesChannel payload before dispatch. */
export function isUpdatesChannel(value: unknown): value is Contract.UpdatesChannel {
  return value === 'stable' || value === 'beta';
}
/** Validate a bounded UpdatesFailure payload before dispatch. */
export function isUpdatesFailure(value: unknown): value is Contract.UpdatesFailure {
  return value === 'check' || value === 'update';
}
/** Validate a bounded UpdatesPhase payload before dispatch. */
export function isUpdatesPhase(value: unknown): value is Contract.UpdatesPhase {
  return (
    value === 'check' ||
    value === 'download' ||
    value === 'verify' ||
    value === 'install' ||
    value === 'start'
  );
}
/** Validate a bounded UpdatesScheduledRestart payload before dispatch. */
export function isUpdatesScheduledRestart(
  value: unknown,
): value is Contract.UpdatesScheduledRestart {
  return value === 'none' || value === 'after-workflow';
}
/** Validate a bounded UpdatesRestartTiming payload before dispatch. */
export function isUpdatesRestartTiming(value: unknown): value is Contract.UpdatesRestartTiming {
  return value === 'now' || value === 'after-workflow';
}
/** Validate a bounded UpdatesRejection payload before dispatch. */
export function isUpdatesRejection(value: unknown): value is Contract.UpdatesRejection {
  return value === 'busy' || value === 'not-ready' || value === 'unavailable';
}
/** Validate a bounded UpdatesCheck payload before dispatch. */
export function isUpdatesCheck(value: unknown): value is Contract.UpdatesCheck {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, 'completedAt') &&
    typeof value['completedAt'] === 'number' &&
    Number.isSafeInteger(value['completedAt']) &&
    value['completedAt'] >= 0 &&
    value['completedAt'] <= 9007199254740991
  );
}
/** Validate a bounded UpdatesStatus payload before dispatch. */
export function isUpdatesStatus(value: unknown): value is Contract.UpdatesStatus {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'state') &&
      value['state'] === 'up-to-date') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'state') &&
      value['state'] === 'checking') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'state') &&
      value['state'] === 'available' &&
      Object.hasOwn(value, 'version') &&
      typeof value['version'] === 'string' &&
      value['version'].length <= 64 &&
      new RegExp('^[a-z0-9.-]{1,64}$').test(value['version'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'state') &&
      value['state'] === 'ready' &&
      Object.hasOwn(value, 'version') &&
      typeof value['version'] === 'string' &&
      value['version'].length <= 64 &&
      new RegExp('^[a-z0-9.-]{1,64}$').test(value['version'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'state') &&
      value['state'] === 'failed' &&
      Object.hasOwn(value, 'during') &&
      isUpdatesFailure(value['during']))
  );
}
/** Validate a bounded UpdatesCause payload before dispatch. */
export function isUpdatesCause(value: unknown): value is Contract.UpdatesCause {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, 'message') &&
    typeof value['message'] === 'string' &&
    value['message'].length <= 2048 &&
    Array.from(value['message']).length <= 1024 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['message'],
    )
  );
}
/** Validate a bounded UpdatesReport payload before dispatch. */
export function isUpdatesReport(value: unknown): value is Contract.UpdatesReport {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'version') &&
    typeof value['version'] === 'string' &&
    value['version'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['version']) &&
    Object.hasOwn(value, 'phase') &&
    isUpdatesPhase(value['phase']) &&
    Object.hasOwn(value, 'causes') &&
    Array.isArray(value['causes']) &&
    value['causes'].length >= 1 &&
    value['causes'].length <= 8 &&
    value['causes'].every((item: unknown) => isUpdatesCause(item))
  );
}
/** Validate a bounded UpdatesState payload before dispatch. */
export function isUpdatesState(value: unknown): value is Contract.UpdatesState {
  return (
    isRecord(value) &&
    Object.keys(value).length ===
      6 +
        (Object.hasOwn(value, 'lastSuccessfulCheck') ? 1 : 0) +
        (Object.hasOwn(value, 'report') ? 1 : 0) &&
    Object.hasOwn(value, 'currentVersion') &&
    typeof value['currentVersion'] === 'string' &&
    value['currentVersion'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['currentVersion']) &&
    Object.hasOwn(value, 'channel') &&
    isUpdatesChannel(value['channel']) &&
    (!Object.hasOwn(value, 'lastSuccessfulCheck') ||
      isUpdatesCheck(value['lastSuccessfulCheck'])) &&
    Object.hasOwn(value, 'status') &&
    isUpdatesStatus(value['status']) &&
    (!Object.hasOwn(value, 'report') || isUpdatesReport(value['report'])) &&
    Object.hasOwn(value, 'previousVersionRunning') &&
    typeof value['previousVersionRunning'] === 'boolean' &&
    Object.hasOwn(value, 'workflowRunning') &&
    typeof value['workflowRunning'] === 'boolean' &&
    Object.hasOwn(value, 'scheduledRestart') &&
    isUpdatesScheduledRestart(value['scheduledRestart'])
  );
}
/** Validate a bounded UpdatesStateMessage payload before dispatch. */
export function isUpdatesStateMessage(value: unknown): value is Contract.UpdatesStateMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'state' &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'update') &&
    isUpdatesState(value['update'])
  );
}
/** Validate a bounded UpdatesRejectedMessage payload before dispatch. */
export function isUpdatesRejectedMessage(value: unknown): value is Contract.UpdatesRejectedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'rejected' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'reason') &&
    isUpdatesRejection(value['reason'])
  );
}
/** Validate a bounded UpdatesAcceptedMessage payload before dispatch. */
export function isUpdatesAcceptedMessage(value: unknown): value is Contract.UpdatesAcceptedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'accepted' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request'])
  );
}
/** Validate a bounded UpdatesWatchMessage payload before dispatch. */
export function isUpdatesWatchMessage(value: unknown): value is Contract.UpdatesWatchMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'watch'
  );
}
/** Validate a bounded UpdatesCheckMessage payload before dispatch. */
export function isUpdatesCheckMessage(value: unknown): value is Contract.UpdatesCheckMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'check' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request'])
  );
}
/** Validate a bounded UpdatesRestartMessage payload before dispatch. */
export function isUpdatesRestartMessage(value: unknown): value is Contract.UpdatesRestartMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.updates' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'restart' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'when') &&
    isUpdatesRestartTiming(value['when'])
  );
}
/** Validate a bounded UpdatesHostMessage payload before dispatch. */
export function isUpdatesHostMessage(value: unknown): value is Contract.UpdatesHostMessage {
  return (
    isUpdatesStateMessage(value) ||
    isUpdatesAcceptedMessage(value) ||
    isUpdatesRejectedMessage(value)
  );
}
/** Validate a bounded UpdatesClientMessage payload before dispatch. */
export function isUpdatesClientMessage(value: unknown): value is Contract.UpdatesClientMessage {
  return (
    isUpdatesWatchMessage(value) || isUpdatesCheckMessage(value) || isUpdatesRestartMessage(value)
  );
}
/** Validate a bounded UpdatesMessage payload before dispatch. */
export function isUpdatesMessage(value: unknown): value is Contract.UpdatesMessage {
  return (
    isUpdatesStateMessage(value) ||
    isUpdatesAcceptedMessage(value) ||
    isUpdatesRejectedMessage(value) ||
    isUpdatesWatchMessage(value) ||
    isUpdatesCheckMessage(value) ||
    isUpdatesRestartMessage(value)
  );
}
