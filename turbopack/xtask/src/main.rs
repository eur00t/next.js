use std::{
    collections::{HashMap, HashSet},
    env::{current_dir, var_os},
    path::PathBuf,
    process,
};

use anyhow::Result;
use clap::{Command, CommandFactory, FromArgMatches, arg};

mod command;
mod nft_bench;
mod patch_package_json;
mod publish;
mod summarize_bench;
mod visualize_bundler_bench;

use nft_bench::show_result;
use patch_package_json::PatchPackageJsonArgs;
use publish::{publish_workspace, run_bump, run_publish};

fn cli() -> Command {
    Command::new("xtask")
        .about("turbo-tooling cargo tasks")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("npm")
                .about("Publish binaries to npm")
                .arg(arg!(<NAME> "the package to publish"))
                .arg_required_else_help(true),
        )
        .subcommand(
            Command::new("workspace")
                .arg(arg!(--publish "publish npm packages in pnpm workspace"))
                .arg(arg!(--bump "bump new version for npm package in pnpm workspace"))
                .arg(arg!(--"dry-run" "dry run all operations"))
                .arg(arg!([NAME] "the package to bump"))
                .about("Manage packages in pnpm workspaces"),
        )
        .subcommand(
            Command::new("nft-bench-result")
                .about("Print node-file-trace benchmark result against @vercel/nft"),
        )
        .subcommand(
            Command::new("upgrade-swc").about("Upgrade all SWC dependencies to the latest version"),
        )
        .subcommand(
            Command::new("summarize-benchmarks")
                .about(
                    "Normalize all raw data based on similar benchmarks, average data by \
                     system+sha and compute latest by system",
                )
                .arg(arg!(<PATH> "the path to the benchmark data directory")),
        )
        .subcommand(
            Command::new("visualize-bundler-benchmarks")
                .about("Generate visualizations of bundler benchmarks")
                .long_about(
                    "Generates visualizations of bundler benchmarks. Currently supports:
    * Scaling: shows how each bundler scales with varying module counts

To generate the summary json file:
    * Check out this repository at the `benchmark-data` branch. An additional shallow clone or git \
                     worktree is recommended.
    * Run `cargo xtask summarize-benchmarks path/to/repo/data`
    * A summary file is generated within the data dir, e.g.  \
                     path/to/repo/data/ubuntu-latest-16-core.json

Visualizations generated by this command will appear in a sibling directory to the summary data \
                     file.",
                )
                .arg(arg!(<PATH_TO_SUMMARY_JSON> "the path to the benchmark summary json file"))
                .arg(arg!(--bundlers <BUNDLERS> "comma separated list of bundlers to include in the visualization")),
        )
        .subcommand(PatchPackageJsonArgs::command())
}

fn main() -> Result<()> {
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("npm", sub_matches)) => {
            let name = sub_matches
                .get_one::<String>("NAME")
                .expect("NAME is required");
            run_publish(name);
            Ok(())
        }
        Some(("workspace", sub_matches)) => {
            let is_bump = sub_matches.get_flag("bump");
            let is_publish = sub_matches.get_flag("publish");
            let dry_run = sub_matches.get_flag("dry-run");
            if is_bump {
                let names = sub_matches
                    .get_many::<String>("NAME")
                    .map(|names| names.cloned().collect::<HashSet<_>>())
                    .unwrap_or_default();
                run_bump(names, dry_run);
            }
            if is_publish {
                publish_workspace(dry_run);
            }
            Ok(())
        }
        Some(("nft-bench-result", _)) => {
            show_result();
            Ok(())
        }
        Some(("upgrade-swc", _)) => {
            let workspace_dir = var_os("CARGO_WORKSPACE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| current_dir().unwrap());
            let cargo_lock_path = workspace_dir.join("../../Cargo.lock");
            let lock = cargo_lock::Lockfile::load(cargo_lock_path).unwrap();
            let swc_packages = lock
                .packages
                .iter()
                .filter(|p| {
                    p.name.as_str().starts_with("swc_")
                        || p.name.as_str() == "swc"
                        || p.name.as_str() == "testing"
                })
                .collect::<Vec<_>>();
            let only_swc_set = swc_packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<HashSet<_>>();
            let packages = lock
                .packages
                .iter()
                .map(|p| (format!("{}@{}", p.name, p.version), p))
                .collect::<HashMap<_, _>>();
            let mut queue = swc_packages.clone();
            let mut set = HashSet::new();
            while let Some(package) = queue.pop() {
                for dep in package.dependencies.iter() {
                    let ident = format!("{}@{}", dep.name, dep.version);
                    let package = *packages.get(&ident).unwrap();
                    if set.insert(ident) {
                        queue.push(package);
                    }
                }
            }
            let status = process::Command::new("cargo")
                .arg("upgrade")
                .arg("--workspace")
                .args(only_swc_set)
                .current_dir(&workspace_dir)
                .stdout(process::Stdio::inherit())
                .stderr(process::Stdio::inherit())
                .status()
                .expect("Running cargo upgrade failed");
            assert!(status.success());
            let status = process::Command::new("cargo")
                .arg("update")
                .args(set.iter().flat_map(|p| ["-p", p]))
                .current_dir(&workspace_dir)
                .stdout(process::Stdio::inherit())
                .stderr(process::Stdio::inherit())
                .status()
                .expect("Running cargo update failed");
            assert!(status.success());
            Ok(())
        }
        Some(("summarize-benchmarks", sub_matches)) => {
            let path = sub_matches
                .get_one::<String>("PATH")
                .expect("PATH is required");
            let path = PathBuf::from(path);
            let path = path.canonicalize().unwrap();
            summarize_bench::process_all(path);
            Ok(())
        }
        Some(("visualize-bundler-benchmarks", sub_matches)) => {
            let path = sub_matches
                .get_one::<String>("PATH_TO_SUMMARY_JSON")
                .expect("PATH_TO_SUMMARY_JSON is required");
            let bundlers: Option<HashSet<&str>> = sub_matches
                .get_one::<String>("bundlers")
                .map(|s| s.split(',').collect());

            let path = PathBuf::from(path);
            let path = path.canonicalize().unwrap();
            visualize_bundler_bench::generate(path, bundlers)
        }
        Some(("patch-package-json", sub_matches)) => {
            patch_package_json::run(&PatchPackageJsonArgs::from_arg_matches(sub_matches)?)
        }
        _ => {
            anyhow::bail!("Unknown command {:?}", matches.subcommand().map(|c| c.0));
        }
    }
}
