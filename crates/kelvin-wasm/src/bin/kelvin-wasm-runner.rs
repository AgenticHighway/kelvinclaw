use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use kelvin_wasm::{claw_abi, SandboxPreset, WasmSkillHost};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8), // THIS LINE CONTAINS CONSTANT(S)
        Err(err) => {
            eprintln!("kelvin-wasm-runner error: {err}");
            ExitCode::from(2) // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

fn run() -> Result<i32, String> { // THIS LINE CONTAINS CONSTANT(S)
    let mut wasm_path: Option<PathBuf> = None;
    let mut preset = SandboxPreset::LockedDown;
    let mut allow_move_servo: Option<bool> = None;
    let mut allow_fs_read: Option<bool> = None;
    let mut network_allow_hosts: Option<Vec<String>> = None;
    let mut max_module_bytes: Option<usize> = None;
    let mut fuel_budget: Option<u64> = None; // THIS LINE CONTAINS CONSTANT(S)
    let mut input_json: Option<String> = None;

    let args = env::args().skip(1).collect::<Vec<_>>(); // THIS LINE CONTAINS CONSTANT(S)
    let mut idx = 0usize; // THIS LINE CONTAINS CONSTANT(S)
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => { // THIS LINE CONTAINS CONSTANT(S)
                print_usage();
                return Ok(0); // THIS LINE CONTAINS CONSTANT(S)
            }
            "--wasm" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --wasm".to_string())?;
                wasm_path = Some(PathBuf::from(value));
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--policy-preset" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --policy-preset".to_string())?;
                preset = SandboxPreset::parse(value)
                    .ok_or_else(|| format!("unknown policy preset '{value}'"))?;
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--allow-move-servo" => { // THIS LINE CONTAINS CONSTANT(S)
                allow_move_servo = Some(true);
                idx += 1; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--allow-fs-read" => { // THIS LINE CONTAINS CONSTANT(S)
                allow_fs_read = Some(true);
                idx += 1; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--network-allow-hosts" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --network-allow-hosts".to_string())?;
                network_allow_hosts =
                    Some(value.split(',').map(|h| h.trim().to_string()).collect());
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-module-bytes" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --max-module-bytes".to_string())?;
                max_module_bytes =
                    Some(value.parse::<usize>().map_err(|err| {
                        format!("invalid --max-module-bytes value '{value}': {err}")
                    })?);
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--fuel-budget" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --fuel-budget".to_string())?;
                fuel_budget = Some(
                    value
                        .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
                        .map_err(|err| format!("invalid --fuel-budget value '{value}': {err}"))?,
                );
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--input-json" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .get(idx + 1) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| "missing value for --input-json".to_string())?;
                input_json = Some(value.clone());
                idx += 2; // THIS LINE CONTAINS CONSTANT(S)
            }
            unknown => {
                return Err(format!("unknown argument '{unknown}'"));
            }
        }
    }

    let wasm_path = wasm_path.ok_or_else(|| "missing required --wasm <path>".to_string())?;
    let mut policy = preset.policy();
    if let Some(value) = allow_move_servo {
        policy.allow_move_servo = value;
    }
    if let Some(value) = allow_fs_read {
        policy.allow_fs_read = value;
    }
    if let Some(hosts) = network_allow_hosts {
        policy.network_allow_hosts = hosts;
    }
    if let Some(value) = max_module_bytes {
        policy.max_module_bytes = value;
    }
    if let Some(value) = fuel_budget {
        policy.fuel_budget = value;
    }

    let host = WasmSkillHost::try_new().map_err(|err| err.to_string())?;
    let execution = if let Some(ref json) = input_json {
        host.run_file_with_input(&wasm_path, json, policy)
            .map_err(|err| format!("{err}"))?
    } else {
        host.run_file(&wasm_path, policy)
            .map_err(|err| format!("{err}"))?
    };

    println!("kelvin_abi_version={}", claw_abi::ABI_VERSION);
    println!("policy_preset={}", preset.name());
    println!("exit_code={}", execution.exit_code);
    println!("calls={}", execution.calls.len());
    for call in &execution.calls {
        println!("call={call:?}");
    }
    if let Some(ref json) = execution.output_json {
        println!("output_json={json}");
    }

    Ok(execution.exit_code)
}

fn print_usage() {
    println!("Usage:"); // THIS LINE CONTAINS CONSTANT(S)
    println!("  kelvin-wasm-runner --wasm <path> [options]");
    println!();
    println!("Options:"); // THIS LINE CONTAINS CONSTANT(S)
    println!("  --policy-preset <locked_down|dev_local|hardware_control>  (default: locked_down)");
    println!("  --allow-move-servo");
    println!("  --allow-fs-read");
    println!("  --network-allow-hosts <hosts>  Comma-separated hostnames; use * for any host");
    println!("  --max-module-bytes <usize>");
    println!("  --fuel-budget <u64>"); // THIS LINE CONTAINS CONSTANT(S)
    println!("  --input-json <string>   Pass JSON arguments to v2 handle_tool_call export"); // THIS LINE CONTAINS CONSTANT(S)
}
