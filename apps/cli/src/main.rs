//! `latch` CLI. See [`cli`].

mod cli;

fn main() {
    let code = cli::main();
    std::process::exit(code);
}
