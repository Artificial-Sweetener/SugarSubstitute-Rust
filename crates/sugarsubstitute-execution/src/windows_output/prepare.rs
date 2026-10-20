// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create owned output pipes and the identity-specific input after execution admission.
use super::*;
impl Lease {
    pub(super) fn prepare_handles(
        &mut self,
        resource: OutputIdentity,
    ) -> Result<(), NativeObservationError> {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: true.into(),
            lpSecurityDescriptor: std::ptr::null_mut(),
        };
        if matches!(resource, OutputIdentity::Managed(_)) {
            let mut read = HANDLE::default();
            let mut write = HANDLE::default();
            // SAFETY: both synchronous pipe outputs are retained immediately. Only the
            // read end inherits; the worker exclusively owns the shutdown writer.
            let result = unsafe { CreatePipe(&mut read, &mut write, Some(&attributes), 0) };
            if !read.is_invalid() {
                self.child[0].adopt(read, "output_create_stdin")?;
            }
            if !write.is_invalid() {
                self.input_writer.adopt(write, "output_create_stdin")?;
            }
            result.map_err(|cause| error("output_create_stdin", cause))?;
            // SAFETY: this exact owned pipe end must never escape to a child, so EOF
            // remains tied to this output owner's shutdown or process death.
            unsafe {
                SetHandleInformation(
                    self.input_writer.raw(),
                    HANDLE_FLAG_INHERIT.0,
                    Default::default(),
                )
            }
            .map_err(|cause| error("output_stdin_no_inherit", cause))?;
        } else {
            let nul = [b'N' as u16, b'U' as u16, b'L' as u16, 0];
            // SAFETY: explicit NUL input and inheritable attributes; output sessions
            // never read ambient stdin or grant write access through this child input.
            let stdin = unsafe {
                CreateFileW(
                    PCWSTR(nul.as_ptr()),
                    FILE_GENERIC_READ.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    Some(&attributes),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(|e| error("output_open_nul", e))?;
            self.child[0].adopt(stdin, "output_open_nul")?;
        }
        let mut random = [0u8; 16];
        // SAFETY: system CSPRNG fills only this bounded non-secret name nonce.
        unsafe {
            windows::Win32::Security::Cryptography::BCryptGenRandom(
                None,
                &mut random,
                windows::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        }
        .ok()
        .map_err(|e| error("output_pipe_name_random", e))?;
        // SAFETY: current process identity is a kernel fact used for own-client validation.
        let pid = unsafe { GetCurrentProcessId() };
        for index in 0..2 {
            let mut name = [0u16; 128];
            let prefix = b"\\\\.\\pipe\\SugarSubstitute-output-";
            for (slot, byte) in name.iter_mut().zip(prefix) {
                *slot = *byte as u16;
            }
            let hex = b"0123456789abcdef";
            for (offset, byte) in random.iter().enumerate() {
                name[prefix.len() + 2 * offset] = hex[(byte >> 4) as usize] as u16;
                name[prefix.len() + 2 * offset + 1] = hex[(byte & 15) as usize] as u16;
            }
            name[prefix.len() + 32] = b'-' as u16;
            name[prefix.len() + 33] = b'0' as u16 + index as u16;
            // SAFETY: exact bounded local name, one instance, inbound overlapped
            // byte pipe. Default token DACL applies; endpoint is verified below.
            let read = unsafe {
                CreateNamedPipeW(
                    PCWSTR(name.as_ptr()),
                    PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    0,
                    MAX_BATCH_BYTES as u32,
                    0,
                    None,
                )
            };
            self.read[index].adopt(read, "output_create_read_pipe")?;
            // SAFETY: we open the sole synchronous write endpoint ourselves;
            // this exact inheritable handle is later copied only by HANDLE_LIST.
            let write = unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    FILE_GENERIC_WRITE.0,
                    FILE_SHARE_MODE(0),
                    Some(&attributes),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(|e| error("output_open_child_write", e))?;
            self.child[index + 1].adopt(write, "output_open_child_write")?;
            let mut actual_pid = 0;
            // SAFETY: query the exact connected pipe before accepting any source bytes.
            unsafe { GetNamedPipeClientProcessId(self.read[index].raw(), &mut actual_pid) }
                .map_err(|e| error("output_verify_own_client", e))?;
            if actual_pid != pid {
                return Err(invariant("output_foreign_pipe_client"));
            }
            let pipe = self.read[index].raw();
            self.operations[index]
                .as_mut()
                .ok_or_else(|| invariant("output_connect_owner"))?
                .connect_owned(pipe)?;
        }
        Ok(())
    }
}
