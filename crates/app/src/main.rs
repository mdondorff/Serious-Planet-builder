fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    if let Err(e) = planet::run(&args, &mut stdout) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
