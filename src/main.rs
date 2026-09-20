use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mdlint::rules::lint;

#[derive(Parser)]
#[command(
    name = "mdlint",
    about = "Lints a Markdown file for common style footguns"
)]
struct Cli {
    /// Markdown file(s) to lint.
    files: Vec<PathBuf>,
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    if cli.files.is_empty() {
        anyhow::bail!("give at least one Markdown file, e.g. `mdlint README.md`");
    }

    let mut total = 0;
    for path in &cli.files {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let findings = lint(&content);
        for f in &findings {
            println!("{}:{}: [{}] {}", path.display(), f.line, f.rule, f.message);
        }
        total += findings.len();
    }

    if total == 0 {
        println!("no issues found");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\n{total} issue(s) found");
        Ok(ExitCode::from(1))
    }
}
