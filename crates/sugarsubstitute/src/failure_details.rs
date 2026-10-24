// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded error causes for installer and stable-entry diagnostics.
use std::{error::Error, fmt};

/// Borrow at most sixteen causes for native dialogs and structured diagnostics.
pub(crate) struct Details<'a>(pub(crate) &'a (dyn Error + 'static));
impl fmt::Display for Details<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut current = Some(self.0);
        for index in 0..16 {
            let Some(cause) = current else { break };
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{cause}")?;
            current = cause.source();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Nested(std::io::Error);
    impl fmt::Display for Nested {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Installation lookup failed")
        }
    }
    impl Error for Nested {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn underlying_failure_remains_visible_in_native_entry_details() {
        let cause = Nested(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Selection access denied",
        ));
        assert_eq!(
            Details(&cause).to_string(),
            "Installation lookup failed\nSelection access denied"
        );
    }

    #[derive(Debug)]
    struct Cyclic;
    impl fmt::Display for Cyclic {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("cyclic")
        }
    }
    impl Error for Cyclic {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(self)
        }
    }
    #[test]
    fn cyclic_failure_cannot_stall_the_error_dialog() {
        assert_eq!(Details(&Cyclic).to_string().lines().count(), 16);
    }
}
