use anyhow::Result;

fn main() -> Result<()> {
    // Dispatch to the modular CLI handler in the SDK.
    // This allows for better maintenance and separate testing of CLI commands.
    zen_engine::cli::main_impl()
}
