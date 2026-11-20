//! M0 skeleton — proves the workspace builds and the facade crate is
//! importable. The window and loop arrive in M2; gameplay in M9.

fn main() {
    // Same logger setup M2's real main() will use.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!(
        "topdown-demo: engine v{} — M0 skeleton OK.",
        engine::core::VERSION
    );
}