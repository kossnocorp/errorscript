use anyhow::{Context, Result, bail};
use errorscript_lib::{Options, generate};

fn main() -> Result<()> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ts-libs" => {
                options.ts_libs = args.next().context("--ts-libs needs a directory")?.into()
            }
            "--node-types" => {
                options.node_types = args
                    .next()
                    .context("--node-types needs a directory")?
                    .into()
            }
            "--node-version" => {
                options.node_version = args.next().context("--node-version needs a major number")?
            }
            "--out" => options.output = args.next().context("--out needs a directory")?.into(),
            "--check" => options.check = true,
            "--help" | "-h" => {
                println!(
                    "errs-lib [--ts-libs DIR] [--node-types DIR] [--node-version 24] [--out DIR] [--check]\n\nDefaults: packaged development TS libraries, pkgs/lib/node_modules/@types/node, pkgs/lib/tests.\n--check reports stale outputs without writing."
                );
                return Ok(());
            }
            _ => bail!("Unknown argument: {arg}"),
        }
    }
    let report = generate(&options)?;
    for path in &report.changed {
        println!(
            "{} {}",
            if options.check { "stale" } else { "updated" },
            path.display()
        );
    }
    println!(
        "{} test files; {} changed outputs",
        report.files,
        report.changed.len()
    );
    if options.check && !report.changed.is_empty() {
        bail!("Generated library tests are out of date");
    }
    Ok(())
}
