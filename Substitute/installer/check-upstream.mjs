// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Qualify an already available upstream revision without changing Git state or contacting a remote. */
import { execFileSync } from 'node:child_process'
import fs from 'node:fs/promises'
import path from 'node:path'
const [revision, output] = process.argv.slice(2)
if (!revision || !output || !path.isAbsolute(output) || process.argv.length !== 4) {
  throw new Error('Usage: node installer/check-upstream.mjs <local-revision> <new-artifact-directory>')
}
await fs.mkdir(output)
const env = { ...process.env, SUBSTITUTE_COMFY_REVISION: revision, SUBSTITUTE_INSTALLER_BUILD: output }
for (const script of ['build.mjs', 'typecheck.mjs']) {
  execFileSync(process.execPath, [path.join('installer', script)], { env, windowsHide: true, stdio: 'inherit' })
}
const receipt = JSON.parse(await fs.readFile(path.join(output, 'worker/build-receipt.json'), 'utf8'))
await fs.writeFile(path.join(output, 'upstream-review.json'), JSON.stringify({
  schema_version: 1, upstream_revision: receipt.upstream_revision, patches: receipt.patches,
  patch_application: 'passed', consumed_types: 'passed', native_qualification: 'required',
  production_pin_changed: false,
}, null, 2))
console.log(`Local upstream compatibility report: ${path.join(output, 'upstream-review.json')}`)
