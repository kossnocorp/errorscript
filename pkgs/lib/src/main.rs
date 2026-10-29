use anyhow::{Context, Result, bail, ensure};
use errorscript_lib::{Options, PendingOptions, generate, next_pending};

fn main() -> Result<()> {
    let mut options = Options::default();
    let mut pending = PendingOptions::default();
    let mut next = false;
    let mut generation_input = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ts-libs" => {
                generation_input = true;
                options.ts_libs = args.next().context("--ts-libs needs a directory")?.into()
            }
            "--node-types" => {
                generation_input = true;
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
            "--next" => {
                next = true;
                pending.limit = args
                    .next()
                    .context("--next needs a positive integer")?
                    .parse()
                    .context("--next needs a positive integer")?;
            }
            "--kind" => pending.kind = Some(args.next().context("--kind needs ts or node")?),
            "--lib" => pending.lib = Some(args.next().context("--lib needs a library stem")?),
            "--after" => pending.after = Some(args.next().context("--after needs a callable ID")?),
            "--help" | "-h" => {
                println!(
                    "errs-lib [--ts-libs DIR] [--node-types DIR] [--node-version 24] [--out DIR] [--check]\nerrs-lib --next NUM [--kind ts|node] [--lib STEM] [--after ID] [--node-version 24] [--out DIR]\n\nDefaults: packaged development TS libraries, pkgs/lib/node_modules/@types/node, pkgs/lib/tests.\n--check reports stale outputs without writing.\n--next prints a JSON array of pending callables, ordered by stable ID, without generating.\n--lib matches an exact library stem (es2015.reflect, fs/promises).\n--after is an exclusive ID cursor, including after that callable is completed.\nListing uses ts and the selected Node major; --kind narrows it. Evidence paths are relative to pkgs/lib/docs/evidence."
                );
                return Ok(());
            }
            _ => bail!("Unknown argument: {arg}"),
        }
    }
    if next {
        ensure!(
            !options.check && !generation_input,
            "--next cannot be combined with --check, --ts-libs, or --node-types"
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&next_pending(&options, &pending)?)?
        );
        return Ok(());
    }
    ensure!(
        pending.kind.is_none() && pending.lib.is_none() && pending.after.is_none(),
        "--kind, --lib, and --after require --next"
    );
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
