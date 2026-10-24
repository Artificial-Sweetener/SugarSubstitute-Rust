// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Explicit producing-build capabilities for isolated native qualification. */
export function qualificationFeatures(selection) {
  if (selection === undefined) return []
  if (!['--local-k1', '--local-updates'].includes(selection)) throw new Error('Unknown native qualification selection')
  const features = ['sugarsubstitute-supervision/local-k1-symbol-trust', 'k1-native-qualification']
  if (selection === '--local-updates') features.push('sugarsubstitute-supervision/local-update-qualification')
  return features
}
