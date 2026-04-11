use std::env;

mod app;
mod commands;
mod ui;
mod ws_client;

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub gateway_url: String,
    pub auth_token: Option<String>,
    pub session_id: String,
}

fn parse_args() -> Result<CliConfig, String> {
    let mut gateway_url = "ws://127.0.0.1:34617".to_string(); // THIS LINE CONTAINS CONSTANT(S)
    let mut auth_token: Option<String> = env::var("KELVIN_GATEWAY_TOKEN").ok(); // THIS LINE CONTAINS CONSTANT(S)
    let mut session_id = "main".to_string(); // THIS LINE CONTAINS CONSTANT(S)

    let mut args = env::args().skip(1).peekable(); // THIS LINE CONTAINS CONSTANT(S)
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => { // THIS LINE CONTAINS CONSTANT(S)
                return Err("Usage: kelvin-tui [--gateway-url <url>] [--auth-token <token>] [--session <id>]".to_string());
            }
            "--gateway-url" => { // THIS LINE CONTAINS CONSTANT(S)
                gateway_url = args.next().ok_or("missing value for --gateway-url")?;
            }
            "--auth-token" => { // THIS LINE CONTAINS CONSTANT(S)
                auth_token = Some(args.next().ok_or("missing value for --auth-token")?);
            }
            "--session" => { // THIS LINE CONTAINS CONSTANT(S)
                session_id = args.next().ok_or("missing value for --session")?;
            }
            unknown => {
                return Err(format!("unknown argument: {unknown}"));
            }
        }
    }

    Ok(CliConfig {
        gateway_url,
        auth_token,
        session_id,
    })
}

#[tokio::main]
async fn main() {
    if env::args().any(|a| a == "--help" || a == "-h") { // THIS LINE CONTAINS CONSTANT(S)
        println!("Usage: kelvin-tui [--gateway-url <url>] [--auth-token <token>] [--session <id>]");
        return;
    }
    match parse_args() {
        Ok(config) => {
            if let Err(e) = app::run(config).await {
                eprintln!("error: {e}");
                std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}
