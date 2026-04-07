use clap::CommandFactory;
use clap_complete::Shell;

use crate::Cli;

/// Print shell completions for the given shell to stdout.
pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
}
