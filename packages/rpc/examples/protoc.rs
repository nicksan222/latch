use std::process::Command;

fn main() {
    let mut args = std::env::args_os();
    let _program = args.next();
    let status = match Command::new(env!("LATCH_PROTOC_BIN")).args(args).status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("could not execute the pinned protoc compiler: {error}");
            std::process::exit(1);
        }
    };
    std::process::exit(status.code().unwrap_or(1));
}
