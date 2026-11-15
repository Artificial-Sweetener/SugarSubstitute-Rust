// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Local-update qualification builds only: a proof serves SugarSubstitute's changelog from a
//! loopback fixture and names its port through one environment variable, so a proof never
//! reaches GitHub. Shipped builds exclude this module and always read the published changelog.
//!
//! The notes are fetched in the desktop child, and `SUGARSUBSTITUTE_`-prefixed variables never
//! reach a child process, so the variable carries no product prefix.

const PORT: &str = "QUALIFICATION_RELEASE_NOTES_PORT";

/// The loopback port a proof named for the changelog, when it named exactly one nonzero port.
pub(super) fn loopback_port() -> Option<u16> {
    port(std::env::var(PORT).ok().as_deref())
}

/// Exactly one nonzero port; any other text names none.
fn port(value: Option<&str>) -> Option<u16> {
    value?.parse::<u16>().ok().filter(|port| *port != 0)
}

#[cfg(test)]
mod tests {
    use super::port;

    #[test]
    fn only_one_nonzero_port_names_the_notes_fixture() {
        assert_eq!(port(Some("8472")), Some(8472));
        for refused in
            [None, Some(""), Some("0"), Some("65536"), Some("8472 "), Some("http://127.0.0.1:8472")]
        {
            assert_eq!(port(refused), None, "{refused:?}");
        }
    }
}
