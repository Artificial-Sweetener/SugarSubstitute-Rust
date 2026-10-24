// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Assemble an explicit release bundle using the selected native inspection tool and runtimes. */
import { packageApplication } from './release/application-package.mjs'
import { windowsInspector } from './release/windows-imports.mjs'

const [app, cubes, output, release, platform, reader, ...runtimeFiles] = process.argv.slice(2)
if (!reader) throw new Error('Usage: node installer/package.mjs <app-release> <SugarCubes-release> <new-output> <release-id> <windows-x86_64|windows-aarch64> <llvm-readobj> [absolute-runtime-dll ...]')
const inspect = await windowsInspector(reader, platform)
const report = await packageApplication({ app, cubes, output, release, build: process.env.SUBSTITUTE_INSTALLER_BUILD, runtimeFiles }, inspect)
console.log(JSON.stringify({ output, platform, ...report }))
