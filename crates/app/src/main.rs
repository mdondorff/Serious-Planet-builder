fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    planet::crash::install(&args);
    let mut stdout = std::io::stdout();
    if let Err(e) = planet::run(&args, &mut stdout) {
        eprintln!("error: {e}");
        // An error that ends the run of the window leaves a report; a refused option or a failed check needs none.
        if args.first().is_some_and(|a| a == "editor") {
            planet::crash::report("error ending the run", &e, "planet::run", "");
        }
        std::process::exit(1);
    }
}
