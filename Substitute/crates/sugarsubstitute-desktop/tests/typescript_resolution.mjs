// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Let Node load the SugarCubes conformance kit from its TypeScript sources without a build.
// SugarCubes imports its modules by their compiled `.js` names; a `.js` import from a `.ts` module
// resolves to the sibling `.ts` file when one exists. Run with `--experimental-transform-types`.
import { existsSync } from 'node:fs';
import { register } from 'node:module';
import { fileURLToPath } from 'node:url';

export async function resolve(specifier, context, next) {
  if (specifier.endsWith('.js') && context.parentURL?.endsWith('.ts')) {
    const source = new URL(specifier.replace(/\.js$/, '.ts'), context.parentURL);
    if (existsSync(fileURLToPath(source))) return next(source.href, context);
  }
  return next(specifier, context);
}

if (!import.meta.url.includes('?hook')) register(`${import.meta.url}?hook`);
