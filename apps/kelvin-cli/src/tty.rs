use std::io::IsTerminal;

/// Returns true if stdout is a TTY.
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Returns true if stdin is a TTY.
pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// Returns true if interactive mode is appropriate (stdin and stdout are both TTYs).
pub fn is_interactive() -> bool {
    stdin_is_tty() && stdout_is_tty()
}
