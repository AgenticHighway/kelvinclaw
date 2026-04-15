use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::{Cli, CompletionsArgs};

pub fn run(args: CompletionsArgs) -> Result<()> {
    if args.write {
        install_completions(args.shell)
    } else {
        print_completions(args.shell);
        Ok(())
    }
}

pub fn install_for_current_shell() -> Result<()> {
    let shell = Shell::from_env().ok_or_else(|| anyhow::anyhow!("could not detect shell"))?;
    install_completions(shell)
}

fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
}

fn install_completions(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, &name, &mut buf);

    let dest = completion_path(shell)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match shell {
        Shell::PowerShell => {
            // Append to $PROFILE rather than overwrite.
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&dest)
                .with_context(|| format!("failed to open {}", dest.display()))?;
            use std::io::Write;
            f.write_all(&buf)?;
        }
        _ => {
            std::fs::write(&dest, &buf)
                .with_context(|| format!("failed to write {}", dest.display()))?;
        }
    }

    println!("Installed {} completions to: {}", shell, dest.display());

    match shell {
        Shell::Zsh => {
            println!("Add the following to your ~/.zshrc if not already present:");
            println!("  fpath=(~/.zsh/completions $fpath)");
            println!("  autoload -Uz compinit && compinit");
        }
        Shell::Bash => {
            println!("Restart your shell or run: source {}", dest.display());
        }
        _ => {}
    }

    Ok(())
}

fn completion_path(shell: Shell) -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;

    let path = match shell {
        Shell::Bash => {
            let xdg = std::env::var("XDG_DATA_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| home.join(".local").join("share"));
            xdg.join("bash-completion")
                .join("completions")
                .join("kelvin")
        }
        Shell::Zsh => home.join(".zsh").join("completions").join("_kelvin"),
        Shell::Fish => home
            .join(".config")
            .join("fish")
            .join("completions")
            .join("kelvin.fish"),
        Shell::PowerShell => {
            // Use $PROFILE or a fallback.
            std::env::var("PROFILE")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| {
                    home.join("Documents")
                        .join("PowerShell")
                        .join("Microsoft.PowerShell_profile.ps1")
                })
        }
        Shell::Elvish => home
            .join(".config")
            .join("elvish")
            .join("lib")
            .join("completions.elv"),
        _ => anyhow::bail!("unsupported shell: {}", shell),
    };

    Ok(path)
}
