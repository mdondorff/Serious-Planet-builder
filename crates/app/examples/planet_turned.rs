//! spike/rg-prototype (lead #175 item 3, spike only, never merged): the planet binary with GEN-140's turned lattices on (reading 30), so the viewer composes a flight store's L over the
//! turned base it was written against. Same arguments as `planet`.
fn main() {
    planet_generators::height::set_turned_noise(true);
    let args: Vec<String> = std::env::args().skip(1).collect();
    planet::crash::install(&args);
    let mut stdout = std::io::stdout();
    if let Err(e) = planet::run(&args, &mut stdout) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
