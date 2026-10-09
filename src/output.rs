/************************************************
* File: output.rs
* Author: Michal Švrček
*
* Colored CLI output
*
* ver. 0.1.0
*************************************************/

use colored::Colorize;

pub fn banner() {
    println!(
        "\n{}",
        format!("◆ ConfigSync v{}", env!("CARGO_PKG_VERSION"))
            .bright_blue()
            .bold()
    );
    println!("{}", "────────────────────────────────────────".dimmed());
}
pub fn heading(s: &str) {
    println!("\n{}", s.cyan().bold());
}
pub fn info(s: &str) {
    println!("{} {s}", "[INFO]".bright_blue().bold());
}
pub fn success(s: &str) {
    println!("{} {s}", "[ OK ]".green().bold());
}
pub fn warn(s: &str) {
    println!("{} {s}", "[WARN]".yellow().bold());
}
pub fn error(s: &str) {
    eprintln!("{} {s}", "[ERROR]".red().bold());
}
pub fn step(s: &str) {
    println!("{} {s}", "[STEP]".yellow().bold());
}
