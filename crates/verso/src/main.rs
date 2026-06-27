use verso::cli::{Cli, Commands};

fn main() {
    let mut cli = Cli::parse_args();
    if cli.tool_version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let config_path = cli.config_path_buf();
    let allow_missing_default_config = !cli.config_was_explicit();
    let command = cli.command.take();
    let result = match command {
        Some(Commands::Doctor(args)) => {
            verso::doctor::run(&config_path, allow_missing_default_config, args.json)
        }
        Some(Commands::Init(args)) => verso::init::run(&config_path, &args),
        None => verso::release::run(cli),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
