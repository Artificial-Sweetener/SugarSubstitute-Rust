// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Closed owned-text codes. Rust resolves these through the sole product catalog. */
export type ProgressCode = 'checkingPython' | 'checkingSugarCubes' | 'installingSugarCubes' | 'sugarCubesReady'
export type FailureCode = 'selectPython'
export type Progress =
  | { phase: string; percent: number; statusCode: ProgressCode }
  | { phase: string; percent: number; status: string; textOwner: 'comfy-desktop' }
export type ProgressSink = (event: 'progress', data: Progress) => void

/** Preserve developer evidence and expose only the closed product category to the host. */
export class ProductFailure extends Error {
  constructor(readonly statusCode: FailureCode, diagnostic: string) { super(diagnostic) }
}
