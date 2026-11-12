// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Upstream telemetry is a local diagnostic event, without an external collector. */
type Properties = Record<string, unknown>
export type FeatureFlagValue = string | boolean
export type OpsFlagFetchResult =
  | { kind: 'value'; value: FeatureFlagValue; payload: unknown }
  | { kind: 'unreachable' }
export { bucketError } from '@upstream/../shared/errorBucket'
let listener: (event: string, properties: Properties) => void = () => {}

export function observe(sink: typeof listener): void { listener = sink }
export function emit(event: string, properties: Properties = {}): void {
  listener(event, properties)
}
export function capture(event: string, properties: Properties = {}): void {
  listener(event, properties)
}

/** No remote telemetry or feature-flag service is connected to this installer. */
export async function getOpsFlagResult(
  _key: string, _distinctId: string, _timeoutMs?: number,
  _onLate?: (result: Extract<OpsFlagFetchResult, { kind: 'value' }>) => void,
): Promise<OpsFlagFetchResult> {
  return { kind: 'unreachable' }
}
