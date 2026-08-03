fn main() {
    let code = cadence_mcp::run();
    if code != 0 {
        std::process::exit(code);
    }
}
