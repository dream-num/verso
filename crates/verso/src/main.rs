use verso::cli::Cli;

fn main() {
    let cli = Cli::parse_args();
    if cli.tool_version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if let Err(error) = verso::release::run(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
