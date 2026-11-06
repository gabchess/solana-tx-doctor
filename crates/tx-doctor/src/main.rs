use clap::Parser;

fn main() {
    dotenvy::dotenv().ok();
    match tx_doctor::cli::run(tx_doctor::cli::Cli::parse()) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
