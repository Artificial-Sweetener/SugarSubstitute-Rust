// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect known diagnostic sink capabilities independently of target or feature activation.
use crate::{
    capability_source,
    error::{Failure, Result, require},
    workspace::Dependency,
};
use std::collections::BTreeSet;
use syn::visit::Visit;

pub(super) fn logging_crate(name: &str) -> bool {
    matches!(
        name,
        "tracing_subscriber"
            | "tracing_appender"
            | "log"
            | "env_logger"
            | "pretty_env_logger"
            | "simplelog"
            | "simple_logger"
            | "log4rs"
            | "fern"
            | "slog"
            | "slog_term"
            | "slog_async"
            | "flexi_logger"
            | "syslog"
            | "systemd_journal_logger"
    )
}
fn endpoint(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "println"
            | "eprint"
            | "eprintln"
            | "dbg"
            | "OutputDebugStringA"
            | "OutputDebugStringW"
            | "ReportEventA"
            | "ReportEventW"
            | "EventWrite"
            | "EventWriteString"
            | "syslog"
            | "openlog"
            | "printf"
            | "fprintf"
            | "vprintf"
            | "vfprintf"
            | "puts"
            | "fputs"
            | "perror"
            | "GetStdHandle"
            | "WriteConsoleA"
            | "WriteConsoleW"
    )
}
fn restricted(path: &[String]) -> bool {
    let first = path.first().map(String::as_str).unwrap_or("");
    let last = path.last().map(String::as_str).unwrap_or("");
    path.windows(2).any(|pair| pair == ["diagnostic_storage", "operational"])
        || (logging_crate(first) && (first != "log" || path.len() > 1))
        || endpoint(last)
        || path.starts_with(&["std".into(), "io".into()])
            && matches!(last, "stdout" | "stderr" | "*")
        || first == "tracing"
            && matches!(
                last,
                "Subscriber" | "Dispatch" | "set_global_default" | "set_default" | "with_default"
            )
}
fn fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| endpoint(name) || name != "log" && logging_crate(name))
        || names.contains("io") && (names.contains("stdout") || names.contains("stderr"))
        || names.contains("tracing")
            && ["Subscriber", "Dispatch", "set_global_default", "set_default", "with_default"]
                .iter()
                .any(|name| names.contains(*name))
}
pub(super) fn rust(text: &str, dependencies: &[Dependency]) -> Result<Vec<String>> {
    let mut source = text.to_owned();
    for dependency in dependencies {
        if let Some(alias) = &dependency.rename {
            source.push_str(&format!(
                "\nextern crate {} as {};",
                dependency.name.replace('-', "_"),
                alias.replace('-', "_")
            ));
        }
    }
    let mut findings = capability_source::findings(&source, restricted, fragments, |_| false)?;
    let file = syn::parse_file(text)
        .map_err(|cause| Failure::external("parse diagnostic literals", cause))?;
    let mut literals = Literals::default();
    literals.visit_file(&file);
    findings.extend(literals.0);
    Ok(findings)
}
#[derive(Default)]
struct Literals(BTreeSet<String>);
impl<'ast> Visit<'ast> for Literals {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if function.sig.ident == "main"
            && function.sig.inputs.is_empty()
            && let syn::ReturnType::Type(_, output) = &function.sig.output
        {
            let explicit_status = matches!(output.as_ref(), syn::Type::Never(_))
                || matches!(output.as_ref(), syn::Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "ExitCode"));
            if !explicit_status {
                self.0.insert("automatic stderr termination".into());
            }
        }
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_lit_str(&mut self, value: &'ast syn::LitStr) {
        let literal = value.value();
        if log_name(&literal) {
            self.0.insert("persistent diagnostic name".into());
        }
    }
}
fn log_name(value: &str) -> bool {
    value.ends_with(".log")
        || value.ends_with(".jsonl")
        || matches!(
            value,
            "operational-v1"
                | "CONOUT$"
                | "CONERR$"
                | "/dev/stdout"
                | "/dev/stderr"
                | "/proc/self/fd/1"
                | "/proc/self/fd/2"
        )
}
/// Conservative auxiliary inspection includes import aliases, destructuring and quoted members.
pub(super) fn auxiliary(text: &str) -> Vec<String> {
    let text = without_comments(text);
    let text = text.as_str();
    let tokens: BTreeSet<_> =
        text.split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '$').collect();
    let console = [
        "console.",
        "console[",
        "= console",
        "=console",
        ".console",
        "node:console",
        "['console']",
        "[\"console\"]",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let mut found: BTreeSet<String> = tokens
        .iter()
        .filter(|token| {
            endpoint(token)
                || logging_crate(token)
                || script_logger(token)
                || **token == "console" && console
                || matches!(
                    **token,
                    "stdout"
                        | "stderr"
                        | "createWriteStream"
                        | "debuglog"
                        | "cerr"
                        | "cout"
                        | "clog"
                        | "spdlog"
                        | "glog"
                )
        })
        .map(|token| (*token).into())
        .collect();
    for literal in text.split(['\'', '"', '`']) {
        if log_name(literal) {
            found.insert("persistent diagnostic name".into());
        }
    }
    found.into_iter().collect()
}
/// Comments do not construct capabilities; quoted properties and embedded programs remain inspected.
fn without_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut quote = None;
    while let Some(ch) = chars.next() {
        if let Some(current) = quote {
            output.push(ch);
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    output.push(escaped);
                }
            } else if ch == current {
                quote = None;
            }
        } else if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            output.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = ' ';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            output.push(' ');
        } else {
            output.push(ch);
        }
    }
    output
}
fn script_logger(name: &str) -> bool {
    matches!(
        name,
        "pino"
            | "winston"
            | "bunyan"
            | "loglevel"
            | "electron-log"
            | "log4js"
            | "signale"
            | "consola"
    )
}
/// npm package aliases retain the original logger identity independently of its local binding.
pub(super) fn script_dependencies(text: &str) -> Result<()> {
    let package: serde_json::Value = serde_json::from_str(text)
        .map_err(|cause| Failure::external("parse diagnostic script dependencies", cause))?;
    for table in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        let Some(dependencies) = package.get(table) else { continue };
        let dependencies = dependencies.as_object().ok_or_else(|| {
            Failure::Policy("diagnostics: dependency table must be an object".into())
        })?;
        for (name, version) in dependencies {
            let version = version.as_str().ok_or_else(|| {
                Failure::Policy("diagnostics: dependency identity must be a string".into())
            })?;
            let identity = version
                .strip_prefix("npm:")
                .map(|alias| alias.rsplit_once('@').map_or(alias, |(name, _)| name))
                .unwrap_or(name);
            require(
                !script_logger(identity) && identity != "debug",
                format!("diagnostics: independent script logger dependency {name}"),
            )?;
        }
    }
    Ok(())
}
/// CSS/DOM console names convey no runtime diagnostic capability; inline script endpoints still do.
pub(super) fn html(text: &str) -> Vec<String> {
    auxiliary(text)
        .into_iter()
        .filter(|finding| {
            finding != "console"
                || ["console.", "console[", "= console", "=console", ".console"]
                    .iter()
                    .any(|needle| text.contains(needle))
        })
        .collect()
}
mod exports;
pub(super) use exports::crate_public_raw_exports;
pub(super) use exports::public_raw_exports;
