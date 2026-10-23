// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Distribution builds must preserve compiler identity and inspect bootstrap dependencies. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { nativeConfiguration } from './native-configuration.mjs'
import { requireBootstrapImages } from './windows-imports.mjs'

async function fixture(t) {
  const parent = await fs.realpath(os.tmpdir())
  const root = await fs.mkdtemp(path.join(parent, 'substitute-native-config-'))
  t.after(async () => {
    assert.equal(path.dirname(await fs.realpath(root)), parent)
    await fs.rm(root, { recursive: true })
  })
  const linker = path.join(root, 'link.exe')
  await fs.writeFile(linker, 'inert compiler fixture')
  await fs.writeFile(path.join(root, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.93.1"\n')
  return { root, target: path.join(root, 'target'), linker,
    environment: { CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: linker } }
}

test('fresh distribution targets retain static CRT and exact toolchain identity on reuse', async t => {
  const f = await fixture(t)
  const first = await nativeConfiguration(f.target, 'windows-x86_64', f.root, f.environment)
  assert.equal(first.target, 'x86_64-pc-windows-msvc')
  assert.equal(first.environment.CARGO_ENCODED_RUSTFLAGS, '-C\x1ftarget-feature=+crt-static')
  assert.equal(first.configuration.crt, 'static')
  assert.equal(first.configuration.linker.path, f.linker)
  assert.match(first.configuration.linker.sha256, /^[a-f0-9]{64}$/)
  await fs.writeFile(path.join(f.target, 'compiled-file'), 'retained')
  assert.deepEqual(await nativeConfiguration(f.target, 'windows-x86_64', f.root, f.environment), first)
  assert.equal(await fs.readFile(path.join(f.target, 'compiled-file'), 'utf8'), 'retained')
  assert.equal(f.environment.CARGO_ENCODED_RUSTFLAGS, undefined)
})

test('unowned build trees and changed compiler bytes or Rust pins are preserved and rejected', async t => {
  const f = await fixture(t)
  await fs.mkdir(f.target)
  const old = path.join(f.target, 'old-configuration')
  await fs.writeFile(old, 'dynamic CRT')
  await assert.rejects(nativeConfiguration(f.target, 'windows-x86_64', f.root, f.environment), /fresh target directory/)
  assert.deepEqual(await fs.readdir(f.target), ['old-configuration'])
  assert.equal(await fs.readFile(old, 'utf8'), 'dynamic CRT')
  const fresh = path.join(f.root, 'fresh')
  await nativeConfiguration(fresh, 'windows-x86_64', f.root, f.environment)
  const marker = path.join(fresh, '.substitute-native-build.json')
  const original = await fs.readFile(marker)
  await fs.writeFile(f.linker, 'different compiler')
  await assert.rejects(nativeConfiguration(fresh, 'windows-x86_64', f.root, f.environment), /configuration changed/)
  await fs.writeFile(f.linker, 'inert compiler fixture')
  await fs.writeFile(path.join(f.root, 'rust-toolchain.toml'), '[toolchain]\nchannel = "different"\n')
  await assert.rejects(nativeConfiguration(fresh, 'windows-x86_64', f.root, f.environment), /configuration changed/)
  assert.deepEqual(await fs.readFile(marker), original)
})

test('ambient flag overrides and missing explicit toolchain fail before creating outputs', async t => {
  const f = await fixture(t)
  for (const name of ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'RUSTUP_TOOLCHAIN']) {
    await assert.rejects(nativeConfiguration(f.target, 'windows-x86_64', f.root, { ...f.environment, [name]: 'override' }), /repository-controlled flags/)
  }
  await assert.rejects(nativeConfiguration(f.target, 'windows-x86_64', f.root, {}), /absolute linker path/)
  await assert.rejects(nativeConfiguration(f.target, 'windows-x86_64', f.root, { ...f.environment, CARGO_BUILD_TARGET: 'other' }), /target differs/)
  await assert.rejects(nativeConfiguration(f.target, 'linux-x86_64', f.root, f.environment), /platform required/)
  await assert.rejects(fs.stat(f.target), { code: 'ENOENT' })
})

test('linked output ancestry is rejected without creating files in the other directory', async t => {
  const f = await fixture(t)
  const real = path.join(f.root, 'real'), link = path.join(f.root, 'link')
  await fs.mkdir(real)
  await fs.symlink(real, link, process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(nativeConfiguration(path.join(link, 'target'), 'windows-x86_64', f.root, f.environment), /traverse links/)
  assert.deepEqual(await fs.readdir(real), [])
})

test('every executable bootstrap requires only operating system libraries', async () => {
  const files = new Map([['app.exe', 'app'], ['launcher.exe', 'launcher'], ['updater.exe', 'updater'], ['app.pdb', 'symbols']])
  const seen = []
  const inspect = async file => {
    seen.push(file)
    return { imports: [{ library: 'kernel32.dll', symbols: ['CloseHandle'], ordinals: [] }], exports: [] }
  }
  assert.equal((await requireBootstrapImages({ files }, inspect)).length, 3)
  assert.deepEqual(seen, ['app', 'launcher', 'updater'])
  for (const library of ['vcruntime140.dll', 'vcruntime140_1.dll', 'private.dll']) {
    await assert.rejects(requireBootstrapImages({ files }, async file => file === 'launcher'
      ? { imports: [{ library, symbols: ['Function'], ordinals: [] }], exports: [] }
      : inspect(file)), /separately installed library: launcher.exe/)
  }
  await assert.rejects(requireBootstrapImages({ files: new Map() }, inspect), /executable/)
})
