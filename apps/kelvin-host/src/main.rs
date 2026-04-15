use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use kelvin_core::{KelvinError, PluginSecurityPolicy, RunOutcome};
use kelvin_sdk::{
    run_with_sdk, KelvinCliMemoryMode, KelvinSdkConfig, KelvinSdkModelSelection,
    KelvinSdkRunRequest, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};

mod consts;

#[derive(Debug, Clone)]
struct CliConfig {
    prompt: Option<String>,
    interactive: bool,
    show_help_only: bool,
    session_id: String,
    workspace_dir: PathBuf,
    memory_mode: KelvinCliMemoryMode,
    timeout_ms: u64,
    system_prompt: Option<String>,
    model_provider_plugin_id: Option<String>,
    state_dir: Option<PathBuf>,
    persist_runs: bool,
    max_session_history_messages: usize,
    compact_to_messages: usize,
}

fn usage() -> &'static str {
    "Usage: kelvin-host [--prompt <text>] [--interactive] [--session <id>] [--workspace <dir>] [--memory markdown|in-memory|fallback] [--timeout-ms <ms>] [--system <text>] [--model-provider <plugin_id>] [--state-dir <dir>] [--persist-runs true|false] [--max-session-history <n>] [--compact-to <n>]"
}

fn parse_bool(value: &str, flag: &str) -> Result<bool, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        v if consts::BOOL_TRUE_VALUES.contains(&v) => Ok(true),
        v if consts::BOOL_FALSE_VALUES.contains(&v) => Ok(false),
        _ => Err(format!("invalid boolean value for {flag}: {value}")),
    }
}

fn parse_args_from<I>(args: I) -> Result<CliConfig, String>
where
    I: IntoIterator<Item = String>,
{
    let mut prompt: Option<String> = None;
    let mut interactive = false;
    let mut show_help_only = false;
    let mut session_id = consts::DEFAULT_SESSION_ID.to_string();
    let mut workspace_dir = env::current_dir().map_err(|err| err.to_string())?;
    let mut memory_mode = KelvinCliMemoryMode::Markdown;
    let mut timeout_ms = consts::DEFAULT_TIMEOUT_MS;
    let mut system_prompt: Option<String> = None;
    let mut model_provider_plugin_id: Option<String> = None;
    let mut state_dir: Option<PathBuf> = None;
    let mut persist_runs = true;
    let mut max_session_history_messages = consts::DEFAULT_MAX_SESSION_HISTORY_MESSAGES;
    let mut compact_to_messages = consts::DEFAULT_COMPACT_TO_MESSAGES;

    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            consts::ARG_HELP_LONG | consts::ARG_HELP_SHORT => return Err(usage().to_string()),
            consts::ARG_INTERACTIVE => {
                interactive = true;
            }
            consts::ARG_PROMPT => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --prompt".to_string())?;
                prompt = Some(value);
            }
            consts::ARG_SESSION => {
                session_id = args
                    .next()
                    .ok_or_else(|| "missing value for --session".to_string())?;
            }
            consts::ARG_WORKSPACE => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --workspace".to_string())?;
                workspace_dir = PathBuf::from(value);
            }
            consts::ARG_MEMORY => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --memory".to_string())?;
                memory_mode = KelvinCliMemoryMode::parse(&value);
            }
            consts::ARG_TIMEOUT_MS => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --timeout-ms".to_string())?;
                timeout_ms = value
                    .parse::<u64>()
                    .map_err(|_| "invalid numeric value for --timeout-ms".to_string())?;
            }
            consts::ARG_SYSTEM => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --system".to_string())?;
                system_prompt = Some(value);
            }
            consts::ARG_MODEL_PROVIDER => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --model-provider".to_string())?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("model provider id must not be empty".to_string());
                }
                model_provider_plugin_id = Some(trimmed.to_string());
            }
            consts::ARG_STATE_DIR => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --state-dir".to_string())?;
                state_dir = Some(PathBuf::from(value));
            }
            consts::ARG_PERSIST_RUNS => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --persist-runs".to_string())?;
                persist_runs = parse_bool(&value, consts::ARG_PERSIST_RUNS)?;
            }
            consts::ARG_MAX_SESSION_HISTORY => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-session-history".to_string())?;
                max_session_history_messages = value
                    .parse::<usize>()
                    .map_err(|_| "invalid numeric value for --max-session-history".to_string())?;
            }
            consts::ARG_COMPACT_TO => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --compact-to".to_string())?;
                compact_to_messages = value
                    .parse::<usize>()
                    .map_err(|_| "invalid numeric value for --compact-to".to_string())?;
            }
            other if other == consts::HELP_COMMAND && prompt.is_none() && !interactive => {
                show_help_only = true;
            }
            other if !other.starts_with('-') && prompt.is_none() => {
                prompt = Some(other.to_string());
            }
            unknown => {
                return Err(format!("unknown argument: {unknown}\n{}", usage()));
            }
        }
    }

    if show_help_only && prompt.is_some() {
        return Err(format!(
            "{} must be used by itself when invoked from the shell",
            consts::HELP_COMMAND
        ));
    }

    if !interactive && prompt.is_none() && !show_help_only {
        return Err(format!("missing prompt\n{}", usage()));
    }

    Ok(CliConfig {
        prompt,
        interactive,
        show_help_only,
        session_id,
        workspace_dir,
        memory_mode,
        timeout_ms,
        system_prompt,
        model_provider_plugin_id,
        state_dir,
        persist_runs,
        max_session_history_messages,
        compact_to_messages,
    })
}

fn parse_args() -> Result<CliConfig, String> {
    parse_args_from(env::args().skip(1))
}

fn model_selection_and_policy(
    plugin_id: &Option<String>,
) -> (KelvinSdkModelSelection, PluginSecurityPolicy) {
    if let Some(plugin_id) = plugin_id.clone() {
        (
            KelvinSdkModelSelection::InstalledPlugin { plugin_id },
            PluginSecurityPolicy {
                allow_network_egress: true,
                ..Default::default()
            },
        )
    } else {
        (
            KelvinSdkModelSelection::Echo,
            PluginSecurityPolicy {
                allow_network_egress: true,
                ..Default::default()
            },
        )
    }
}

fn runtime_config_from_cli(config: &CliConfig) -> KelvinSdkRuntimeConfig {
    let (model_provider, plugin_security_policy) =
        model_selection_and_policy(&config.model_provider_plugin_id);
    KelvinSdkRuntimeConfig {
        workspace_dir: config.workspace_dir.clone(),
        default_session_id: config.session_id.clone(),
        memory_mode: config.memory_mode,
        default_timeout_ms: config.timeout_ms,
        default_system_prompt: config.system_prompt.clone(),
        core_version: env!("CARGO_PKG_VERSION").to_string(),
        plugin_security_policy,
        load_installed_plugins: true,
        model_provider,
        require_cli_plugin_tool: true,
        emit_stdout_events: false,
        state_dir: config
            .state_dir
            .clone()
            .or_else(|| Some(config.workspace_dir.join(consts::DEFAULT_STATE_DIR_PATH))),
        persist_runs: config.persist_runs,
        max_session_history_messages: config.max_session_history_messages,
        compact_to_messages: config.compact_to_messages,
        max_tool_iterations: consts::MAX_TOOL_ITERATIONS,
    }
}

fn interactive_help_body() -> String {
    format!(
        "Try asking:\n\
  - What can you help me do here?\n\
  - Summarize the files in this folder\n\
  - Help me make a small change safely\n\
\n\
Commands:\n\
  {}  show this guide again\n\
  {}  quit",
        consts::HELP_COMMAND,
        consts::EXIT_COMMAND_LOWERCASE,
    )
}

fn interactive_help_banner(config: &CliConfig, plugin_count: Option<usize>) -> String {
    let provider = config
        .model_provider_plugin_id
        .as_deref()
        .unwrap_or("echo (no model provider configured)");
    let mut banner = if let Some(plugin_count) = plugin_count {
        format!(
            "interactive mode ready (session='{}', provider={}, plugins={})\n\n{}",
            config.session_id,
            provider,
            plugin_count,
            interactive_help_body(),
        )
    } else {
        format!(
            "Kelvin interactive quickstart (session='{}', provider={})\n\n{}\n\nRun `kelvin` and choose CLI chat to start interactive mode.",
            config.session_id,
            provider,
            interactive_help_body(),
        )
    };
    if config.model_provider_plugin_id.is_none() {
        banner.push_str(
            "\n\nTip: echo mode is useful for smoke tests. Configure a real model provider with `kelvin init` before relying on non-echo responses.",
        );
    }
    banner
}

enum InteractiveMetaCommand {
    NotHandled,
    Continue,
    Exit,
}

fn handle_interactive_meta_command(
    config: &CliConfig,
    prompt: &str,
    plugin_count: usize,
) -> InteractiveMetaCommand {
    if prompt.eq_ignore_ascii_case(consts::HELP_COMMAND) {
        println!("{}", interactive_help_banner(config, Some(plugin_count)));
        return InteractiveMetaCommand::Continue;
    }
    if prompt.eq_ignore_ascii_case(consts::EXIT_COMMAND_LOWERCASE)
        || prompt.eq_ignore_ascii_case(consts::EXIT_COMMAND_QUIT)
    {
        return InteractiveMetaCommand::Exit;
    }
    InteractiveMetaCommand::NotHandled
}

async fn run_single(config: CliConfig) -> Result<(), KelvinError> {
    let (model_provider, plugin_security_policy) =
        model_selection_and_policy(&config.model_provider_plugin_id);
    let result = run_with_sdk(KelvinSdkConfig {
        prompt: config.prompt.unwrap_or_default(),
        session_id: config.session_id,
        workspace_dir: config.workspace_dir.clone(),
        memory_mode: config.memory_mode,
        timeout_ms: config.timeout_ms,
        system_prompt: config.system_prompt,
        core_version: env!("CARGO_PKG_VERSION").to_string(),
        plugin_security_policy,
        load_installed_plugins: true,
        model_provider,
        state_dir: config
            .state_dir
            .or_else(|| Some(config.workspace_dir.join(consts::DEFAULT_STATE_DIR_PATH))),
        persist_runs: config.persist_runs,
        max_session_history_messages: config.max_session_history_messages,
        compact_to_messages: config.compact_to_messages,
        max_tool_iterations: consts::MAX_TOOL_ITERATIONS,
    })
    .await?;

    println!("cli plugin preflight: {}", result.cli_plugin_preflight);
    println!(
        "run complete in {}ms (provider={}, model={})",
        result.duration_ms, result.provider, result.model
    );
    for payload in result.payloads {
        println!("payload: {payload}");
    }
    Ok(())
}

async fn run_interactive(config: CliConfig) -> Result<(), KelvinError> {
    let runtime = KelvinSdkRuntime::initialize(runtime_config_from_cli(&config)).await?;
    let plugin_count = runtime.loaded_installed_plugins();
    println!("{}", interactive_help_banner(&config, Some(plugin_count)));

    if let Some(first_prompt) = config.prompt.clone() {
        match handle_interactive_meta_command(&config, &first_prompt, plugin_count) {
            InteractiveMetaCommand::NotHandled => {
                process_prompt(&runtime, first_prompt, config.timeout_ms).await?;
            }
            InteractiveMetaCommand::Continue => {}
            InteractiveMetaCommand::Exit => return Ok(()),
        }
    }

    let mut stdin = io::stdin().lock();
    let mut buffer = String::new();
    loop {
        print!("{}", consts::INTERACTIVE_PROMPT);
        io::stdout()
            .flush()
            .map_err(|err| KelvinError::Io(format!("flush stdout: {err}")))?;
        buffer.clear();
        let bytes_read = stdin
            .read_line(&mut buffer)
            .map_err(|err| KelvinError::Io(format!("read interactive input: {err}")))?;
        if bytes_read == consts::BYTES_EOF {
            break;
        }
        let prompt = buffer.trim();
        if prompt.is_empty() {
            continue;
        }
        match handle_interactive_meta_command(&config, prompt, plugin_count) {
            InteractiveMetaCommand::NotHandled => {}
            InteractiveMetaCommand::Continue => continue,
            InteractiveMetaCommand::Exit => break,
        }
        process_prompt(&runtime, prompt.to_string(), config.timeout_ms).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config(model_provider_plugin_id: Option<&str>) -> CliConfig {
        CliConfig {
            prompt: None,
            interactive: true,
            show_help_only: false,
            session_id: "main".to_string(),
            workspace_dir: PathBuf::from("."),
            memory_mode: KelvinCliMemoryMode::Markdown,
            timeout_ms: consts::DEFAULT_TIMEOUT_MS,
            system_prompt: None,
            model_provider_plugin_id: model_provider_plugin_id.map(ToString::to_string),
            state_dir: None,
            persist_runs: true,
            max_session_history_messages: consts::DEFAULT_MAX_SESSION_HISTORY_MESSAGES,
            compact_to_messages: consts::DEFAULT_COMPACT_TO_MESSAGES,
        }
    }

    #[test]
    fn interactive_banner_includes_examples_and_commands() {
        let banner = interactive_help_banner(&sample_config(Some("kelvin.openai")), Some(3));
        assert!(banner.contains("interactive mode ready"));
        assert!(banner.contains("provider=kelvin.openai"));
        assert!(banner.contains("What can you help me do here?"));
        assert!(banner.contains(consts::HELP_COMMAND));
        assert!(banner.contains(consts::EXIT_COMMAND_LOWERCASE));
    }

    #[test]
    fn interactive_banner_calls_out_echo_mode() {
        let banner = interactive_help_banner(&sample_config(None), Some(2));
        assert!(banner.contains("provider=echo (no model provider configured)"));
        assert!(banner.contains("echo mode is useful for smoke tests"));
    }

    #[test]
    fn quickstart_banner_mentions_shell_entrypoint() {
        let banner = interactive_help_banner(&sample_config(Some("kelvin.openai")), None);
        assert!(banner.contains("Kelvin interactive quickstart"));
        assert!(banner.contains("Run `kelvin` and choose CLI chat"));
    }

    #[test]
    fn bare_help_command_shows_help_only_mode() {
        let config = parse_args_from(vec![consts::HELP_COMMAND.to_string()]).unwrap();
        assert!(config.show_help_only);
        assert!(!config.interactive);
        assert!(config.prompt.is_none());
    }

    #[test]
    fn interactive_help_command_stays_in_interactive_mode() {
        let config = parse_args_from(vec![
            consts::ARG_INTERACTIVE.to_string(),
            consts::HELP_COMMAND.to_string(),
        ])
        .unwrap();
        assert!(config.interactive);
        assert!(!config.show_help_only);
        assert_eq!(config.prompt.as_deref(), Some(consts::HELP_COMMAND));
    }
}

async fn process_prompt(
    runtime: &KelvinSdkRuntime,
    prompt: String,
    timeout_ms: u64,
) -> Result<(), KelvinError> {
    let accepted = runtime
        .submit(KelvinSdkRunRequest::for_prompt(prompt))
        .await?;
    match runtime
        .wait_for_outcome(
            &accepted.run_id,
            timeout_ms.saturating_add(consts::TIMEOUT_BUFFER_MS),
        )
        .await?
    {
        RunOutcome::Completed(result) => {
            for payload in result.payloads {
                println!("{}", payload.text);
            }
        }
        RunOutcome::Failed(error) => {
            println!("run failed: {error}");
        }
        RunOutcome::Timeout => {
            println!("run timed out");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    match parse_args() {
        Ok(config) => {
            let result = if config.show_help_only {
                println!("{}", interactive_help_banner(&config, None));
                Ok(())
            } else if config.interactive {
                run_interactive(config).await
            } else {
                run_single(config).await
            };
            if let Err(err) = result {
                eprintln!("error: {err}");
                if err.to_string().contains(consts::KELVIN_CLI_PLUGIN_ID) {
                    eprintln!(
                        "hint: install the CLI plugin with: kelvin plugin install kelvin.cli"
                    );
                }
                if err.to_string().contains(consts::OPENAI_API_KEY_VAR) {
                    eprintln!(
                        "hint: set OPENAI_API_KEY and install the OpenAI model plugin with: kelvin plugin install kelvin.openai"
                    );
                }
                if err.to_string().contains(consts::ANTHROPIC_API_KEY_VAR) {
                    eprintln!(
                        "hint: set ANTHROPIC_API_KEY and install the Anthropic model plugin with: kelvin plugin install kelvin.anthropic"
                    );
                }
                std::process::exit(consts::EXIT_FAILURE);
            }
        }
        Err(err) => {
            eprintln!("{err}");
            if err.starts_with(consts::USAGE_PREFIX) {
                std::process::exit(consts::EXIT_SUCCESS);
            }
            std::process::exit(consts::EXIT_FAILURE);
        }
    }
}
