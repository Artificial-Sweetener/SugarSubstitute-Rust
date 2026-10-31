// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Producing-build Cargo arguments and the explicit capabilities of isolated native qualification. */

/**
 * Cargo arguments for one producing build. Every build keeps the application's declared default
 * feature set, which is the shipped set (including native canvas composition); qualification
 * capabilities are only ever added on top of it.
 */
export function nativeBuildArguments(names, target, profile, features) {
  const args = ['build', '--locked', '-p', 'sugarsubstitute', ...names.flatMap(name => ['--bin', name]), '--target', target, '--profile', profile, '--message-format=json-render-diagnostics']
  if (features.length) args.push('--features', features.join(','))
  return args
}

/** Explicit producing-build capabilities for isolated native qualification. */
export function qualificationFeatures(selection) {
  if (selection === undefined) return []
  if (!['--local-k1', '--local-updates'].includes(selection)) throw new Error('Unknown native qualification selection')
  const features = ['sugarsubstitute-supervision/local-k1-symbol-trust', 'k1-native-qualification']
  if (selection === '--local-updates') features.push('sugarsubstitute-supervision/local-update-qualification')
  return features
}
