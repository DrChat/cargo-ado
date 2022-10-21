use std::process::Stdio;

use anyhow::Context;
use cargo_metadata::{diagnostic::DiagnosticLevel, Message};
use clap::{Arg, Command};

fn main() -> anyhow::Result<()> {
    let matches = Command::new("cargo")
        .bin_name("cargo")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Justin Moore <jusmoore@microsoft.com>")
        .about("Run a cargo command and format output for Azure DevOps")
        .subcommand(
            Command::new("ado")
                .arg_required_else_help(true)
                .args([Arg::new("options").num_args(1..).trailing_var_arg(true)]),
        )
        .get_matches();

    let matches = match matches.subcommand() {
        Some(("ado", matches)) => matches,
        _ => unreachable!(),
    };

    let options = matches
        .get_many::<String>("options")
        .unwrap()
        .map(|s| s.as_str());

    let mut command = std::process::Command::new("cargo")
        .args(options.into_iter().chain(["--message-format=json"]))
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to spawn cargo")?;

    let reader = std::io::BufReader::new(command.stdout.take().unwrap());
    for message in Message::parse_stream(reader) {
        match message.context("failed to parse cargo diagnostic message")? {
            Message::CompilerMessage(msg) => {
                let code = match msg.message.code {
                    Some(ref code) => code,
                    // Summary error messages are emitted without an associated code.
                    None => continue,
                };

                let level = match msg.message.level {
                    DiagnosticLevel::Ice => "error",
                    DiagnosticLevel::Error => "error",
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::FailureNote => "error",
                    DiagnosticLevel::Note => "warning",
                    DiagnosticLevel::Help => "warning",
                    _ => panic!("unknown diagnostic level"),
                };

                // HACK: Only reporting the first span or nothing.
                let source = if msg.message.spans.len() == 1 {
                    msg.message.spans.first()
                } else {
                    None
                };

                // https://github.com/microsoft/azure-pipelines-tasks/blob/master/docs/authoring/commands.md
                println!(
                    "##vso[task.logissue type={level}{};code={}]{}",
                    if let Some(source) = source {
                        format!(
                            ";sourcepath={};linenumber={};columnnumber={}",
                            source.file_name, source.line_start, source.column_start
                        )
                    } else {
                        format!("")
                    },
                    code.code,
                    msg.message.message
                );

                if let Some(rendered) = msg.message.rendered {
                    println!("{}", rendered);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
