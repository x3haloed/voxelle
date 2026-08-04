use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_contract_path);
    voxelle_app::write_shell_contract(&output)?;
    let ontology_output = output.with_file_name("ui-ontology.fixture.mjs");
    voxelle_app::write_ui_ontology_fixture(&ontology_output)?;
    println!("{}", output.display());
    println!("{}", ontology_output.display());
    Ok(())
}

fn default_contract_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("web")
        .join("src")
        .join("shell-contract.ts")
}
