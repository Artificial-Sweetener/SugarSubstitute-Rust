// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enter the independently runnable native image viewer on the operating system main thread.
fn main() -> std::process::ExitCode {
    #[cfg(windows)]
    {
        sugarsubstitute::viewer::entry()
    }
    #[cfg(not(windows))]
    {
        std::process::ExitCode::from(78)
    }
}
