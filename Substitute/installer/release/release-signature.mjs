// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** The release-manifest key: RSA-3072, signed as the application verifies it (CNG RSA-PSS). */
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign, verify, constants } from 'node:crypto'

const blobHeader = Buffer.from('52534131000c000003000000800100000000000000000000', 'hex')
const exponent = Buffer.from('010001', 'hex')
const modulusBytes = 384
export const signatureBytes = 384
export const manifestLimit = 64 * 1024
const pss = { padding: constants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }

/** The 411-byte `BCRYPT_RSAPUBLIC_BLOB` the application compiles in as its release anchor. */
export function publicBlob(key) {
  const publicKey = key?.type === 'public' ? key : createPublicKey(key)
  const { n, e } = publicKey.export({ format: 'jwk' })
  const modulus = Buffer.from(n, 'base64url')
  if (modulus.length !== modulusBytes || !Buffer.from(e, 'base64url').equals(exponent)) {
    throw new Error('The release manifest key must be RSA-3072 with exponent 65537')
  }
  return Buffer.concat([blobHeader, exponent, modulus])
}

/** The key identity signed manifests carry: SHA-256 of the public blob, as hexadecimal. */
export function keyId(blob) {
  return createHash('sha256').update(blob).digest('hex')
}

/** The public key an anchor blob encodes, refusing anything but the exact compiled shape. */
export function anchorKey(blob) {
  if (blob.length !== blobHeader.length + exponent.length + modulusBytes ||
      !blob.subarray(0, blobHeader.length).equals(blobHeader) ||
      !blob.subarray(blobHeader.length, blobHeader.length + exponent.length).equals(exponent)) {
    throw new Error('The release anchor is not an RSA-3072 CNG public blob with exponent 65537')
  }
  const n = blob.subarray(blobHeader.length + exponent.length).toString('base64url')
  return createPublicKey({ key: { kty: 'RSA', n, e: exponent.toString('base64url') }, format: 'jwk' })
}

/** A new release manifest key: its PKCS#8 PEM (the secret) and its public anchor blob. */
export function generateReleaseKey() {
  const { privateKey } = generateKeyPairSync('rsa', { modulusLength: 3072, publicExponent: 65537 })
  return { privatePem: privateKey.export({ type: 'pkcs8', format: 'pem' }), publicBlob: publicBlob(privateKey) }
}

/**
 * Sign manifest bytes with the PEM key, only when that key is the one `anchor` names, and
 * verify the signature against the anchor before returning it.
 */
export function signManifest(privatePem, manifest, anchor) {
  if (!Buffer.isBuffer(manifest) || manifest.length === 0 || manifest.length > manifestLimit) {
    throw new Error('The release manifest is empty or exceeds its bound')
  }
  let key
  try { key = createPrivateKey(privatePem) } catch { throw new Error('The release manifest key is not a private key') }
  if (!publicBlob(key).equals(anchor)) throw new Error('The release manifest key does not match the committed release anchor')
  const signature = sign('sha256', manifest, { key, ...pss })
  if (signature.length !== signatureBytes || !verifyManifest(manifest, signature, anchor)) {
    throw new Error('The release manifest signature did not verify under the release anchor')
  }
  return signature
}

/** Verify as the application does: SHA-256, RSA-PSS with a 32-byte salt, under the anchor. */
export function verifyManifest(manifest, signature, anchor) {
  return signature.length === signatureBytes && verify('sha256', manifest, { key: anchorKey(anchor), ...pss }, signature)
}
