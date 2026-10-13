// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build logon-restricted pipe DACLs for exact client endpoint and coordinator server roles.
use super::{Identity, NativePipeError};
use windows::Win32::System::SystemServices::SECURITY_DESCRIPTOR_REVISION;
use windows::Win32::{Security::*, Storage::FileSystem::*};
/// Distinguish endpoint access from the extra rights needed to create more pipe instances.
#[derive(Clone, Copy)]
pub(crate) enum DescriptorPurpose {
    /// Existing one-instance endpoint and explicit restricted client access.
    RestrictedEndpoint,
    /// Actual coordinator server creates all64 instances under the same logon DACL.
    CoordinatorServer,
}
impl DescriptorPurpose {
    fn access_mask(self) -> u32 {
        match self {
            Self::RestrictedEndpoint => {
                FILE_READ_DATA.0
                    | FILE_WRITE_DATA.0
                    | FILE_READ_ATTRIBUTES.0
                    | FILE_WRITE_ATTRIBUTES.0
                    | SYNCHRONIZE.0
            }
            // CreateNamedPipe rechecks duplex generic rights plus CREATE_PIPE_INSTANCE
            // against the first instance's DACL when the same owner creates subsequent ones.
            Self::CoordinatorServer => {
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | FILE_CREATE_PIPE_INSTANCE.0
            }
        }
    }
}
/// Keep ACL storage stable while the owned descriptor moves between admitted phases.
pub(crate) struct Descriptor {
    descriptor: SECURITY_DESCRIPTOR,
    acl: Box<[u32; 128]>,
}
impl Descriptor {
    /// Construct one exact protected logon DACL; callers fund storage before allocation.
    /// # Errors
    /// Preserves actual descriptor/ACL construction failures.
    pub(crate) fn new(
        identity: Identity,
        purpose: DescriptorPurpose,
    ) -> Result<Self, NativePipeError> {
        let mut value =
            Self { descriptor: SECURITY_DESCRIPTOR::default(), acl: Box::new([0; 128]) };
        // SAFETY: descriptor and aligned ACL storage are fully owned through native object creation.
        unsafe {
            InitializeSecurityDescriptor(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                SECURITY_DESCRIPTOR_REVISION,
            )?;
            InitializeAcl(
                value.acl.as_mut_ptr().cast(),
                size_of_val(value.acl.as_ref()) as u32,
                ACL_REVISION,
            )?;
            AddAccessAllowedAceEx(
                value.acl.as_mut_ptr().cast(),
                ACL_REVISION,
                ACE_FLAGS(0),
                purpose.access_mask(),
                PSID(identity.logon.bytes.as_ptr() as *mut _),
            )?;
            SetSecurityDescriptorDacl(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                true,
                Some(value.acl.as_ptr().cast()),
                false,
            )?;
            SetSecurityDescriptorControl(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                SE_DACL_PROTECTED,
                SE_DACL_PROTECTED,
            )?;
        }
        Ok(value)
    }
    /// Borrow creation attributes only while this descriptor and its boxed ACL remain live.
    pub(crate) fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: (&mut self.descriptor as *mut SECURITY_DESCRIPTOR).cast(),
            bInheritHandle: false.into(),
        }
    }
}

#[cfg(test)]
#[path = "descriptor_tests.rs"]
mod tests;
