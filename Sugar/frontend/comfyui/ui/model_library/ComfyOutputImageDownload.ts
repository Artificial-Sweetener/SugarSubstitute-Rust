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
/** Read one approved local Comfy image with bounded transfer and cancellation. */
const IMAGE_LIMIT = 20 * 1024 * 1024;
export interface ComfyOutputImageResponse {
  readonly ok: boolean;
  readonly headers: Pick<Headers, 'get'>;
  readonly body: {
    getReader(): Pick<ReadableStreamDefaultReader<Uint8Array>, 'read' | 'cancel'>;
  } | null;
}
export type ComfyOutputImageFetch = (
  url: string,
  options: RequestInit,
) => Promise<ComfyOutputImageResponse>;

/** Reject late transport completion with the original cancellation or timeout reason. */
function requireActiveImageRead(signal: AbortSignal): void {
  if (signal.aborted) throw signal.reason;
}

/** Preserve approved image bytes and reject cancelled reads before returning an uploadable file. */
export async function readComfyOutputImage(
  url: string,
  filename: string,
  signal: AbortSignal,
  fetchImage: ComfyOutputImageFetch = fetch,
): Promise<File> {
  const target = new URL(url, document.baseURI);
  if (
    target.origin !== new URL(document.baseURI).origin ||
    target.pathname !== '/view' ||
    target.username ||
    target.password ||
    !['output', 'temp'].includes(target.searchParams.get('type') ?? '')
  )
    throw new Error('That image is not a completed Comfy output.');
  requireActiveImageRead(signal);
  const controller = new AbortController();
  /** Cancel the local image request with the mounted field's owning action. */
  const cancel = (): void => controller.abort(signal.reason);
  if (signal.aborted) cancel();
  signal.addEventListener('abort', cancel, { once: true });
  const timeout = setTimeout(
    () => controller.abort(new Error('The output image request timed out.')),
    15_000,
  );
  try {
    const response = await fetchImage(url, {
      signal: controller.signal,
      credentials: 'same-origin',
      redirect: 'error',
    });
    requireActiveImageRead(controller.signal);
    const type = response.headers.get('content-type')?.split(';')[0]?.trim() ?? '';
    if (!response.ok) throw new Error('The output image could not be read.');
    if (!['image/png', 'image/jpeg', 'image/webp', 'image/gif'].includes(type))
      throw new Error('The output is not a supported image.');
    if (Number(response.headers.get('content-length')) > IMAGE_LIMIT)
      throw new Error('The output image exceeds the 20 MiB thumbnail limit.');
    const reader = response.body?.getReader();
    if (!reader) throw new Error('The output image is empty.');
    const parts: ArrayBuffer[] = [];
    let length = 0;
    try {
      let chunk = await reader.read();
      requireActiveImageRead(controller.signal);
      while (!chunk.done) {
        length += chunk.value.byteLength;
        if (length > IMAGE_LIMIT)
          throw new Error('The output image exceeds the 20 MiB thumbnail limit.');
        parts.push(new Uint8Array(chunk.value).buffer);
        chunk = await reader.read();
        requireActiveImageRead(controller.signal);
      }
      if (!length) throw new Error('The output image is empty.');
      return new File(parts, filename, { type });
    } finally {
      await reader.cancel();
    }
  } finally {
    clearTimeout(timeout);
    signal.removeEventListener('abort', cancel);
  }
}
