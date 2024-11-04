use std::io::Write as _;
use std::{fs::File, io::BufRead};

use gen_target_info::{
    get_target_spec_from_msrv, get_target_specs_from_json, get_targets_msrv, RustcTargetSpecs,
};

const PRELUDE: &str = r#"//! This file is generated code. Please edit the generator
//! in dev-tools/gen-target-info if you need to make changes.

"#;

fn generate_target_mapping(f: &mut File, target_specs: &RustcTargetSpecs) -> std::io::Result<()> {
    writeln!(f, "use super::TargetInfo;")?;
    writeln!(f)?;
    writeln!(
        f,
        "pub(crate) const LIST: &[(&str, TargetInfo<'static>)] = &["
    )?;

    for (triple, spec) in &target_specs.0 {
        let full_arch = triple.split_once('-').unwrap().0;
        let arch = &spec.arch;
        let vendor = spec.vendor.as_deref().unwrap_or("unknown");
        let os = spec.os.as_deref().unwrap_or("none");
        let env = spec.env.as_deref().unwrap_or("");
        let abi = spec.abi.as_deref().unwrap_or("");
        let features = spec.cfgs.target_features.join(",");

        let unversioned_llvm_target = if spec.llvm_target.contains("apple") {
            // Remove deployment target information from LLVM target triples (we
            // will add this in another part of CC).
            //
            // FIXME(madsmtm): Should become unnecessary after
            // https://github.com/rust-lang/rust/pull/131037
            let mut components = spec.llvm_target.split("-").collect::<Vec<_>>();

            components[2] = components[2].trim_end_matches(|c: char| c.is_numeric() || c == '.');

            components.join("-")
        } else if os == "uefi" {
            // Override the UEFI LLVM targets.
            //
            // The rustc mappings (as of 1.82) for the UEFI targets are:
            // * i686-unknown-uefi -> i686-unknown-windows-gnu
            // * x86_64-unknown-uefi -> x86_64-unknown-windows
            // * aarch64-unknown-uefi -> aarch64-unknown-windows
            //
            // However, in cc-rs all the UEFI targets use
            // -windows-gnu. This has been the case since 2021 [1].
            // * i686-unknown-uefi -> i686-unknown-windows-gnu
            // * x86_64-unknown-uefi -> x86_64-unknown-windows-gnu
            // * aarch64-unknown-uefi -> aarch64-unknown-windows-gnu
            //
            // For now, override the UEFI mapping to keep the behavior
            // of cc-rs unchanged.
            //
            // TODO: as discussed in [2], it may be possible to switch
            // to new UEFI targets added to clang, and regardless it
            // would be good to have consistency between rustc and
            // cc-rs.
            //
            // [1]: https://github.com/rust-lang/cc-rs/pull/623
            // [2]: https://github.com/rust-lang/cc-rs/pull/1264
            let arch = if spec.arch == "x86" {
                "i686"
            } else {
                &spec.arch
            };
            format!("{}-unknown-windows-gnu", arch)
        } else {
            spec.llvm_target.clone()
        };

        writeln!(f, "    (")?;
        writeln!(f, "        {triple:?},")?;
        writeln!(f, "        TargetInfo {{")?;
        writeln!(f, "            full_arch: {full_arch:?},")?;
        writeln!(f, "            arch: {arch:?},")?;
        writeln!(f, "            vendor: {vendor:?},")?;
        writeln!(f, "            os: {os:?},")?;
        writeln!(f, "            env: {env:?},")?;
        writeln!(f, "            abi: {abi:?},")?;
        writeln!(
            f,
            "            unversioned_llvm_target: {unversioned_llvm_target:?},"
        )?;
        // NOTE: Features are generated from nightly versions, which will
        // result in unstable values being output here as well. That is
        // probably desirable since:
        // 1. They're only used when `cc` is used outside a build script, and
        //    then we can't do feature detection, so we have to pick either
        //    the stable or the nightly representation.
        // 2. The nightly representation is much more feature-ful, and `cc`'s
        //    conversion is going to be best-effort anyhow.
        writeln!(f, "            features: {features:?},")?;
        writeln!(f, "        }},")?;
        writeln!(f, "    ),")?;
    }

    writeln!(f, "];")?;

    Ok(())
}

fn main() {
    // Primarily use information from nightly.
    let mut target_specs = get_target_specs_from_json();
    // Next, read from MSRV to support old, removed targets.
    for target_triple in get_targets_msrv().lines() {
        let target_triple = target_triple.unwrap();
        let target_triple = target_triple.trim();
        target_specs
            .0
            .entry(target_triple.to_string())
            .or_insert_with(|| get_target_spec_from_msrv(target_triple));
    }

    // Open file to write to
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let path = format!("{manifest_dir}/../../src/target/generated.rs");
    let mut f = File::create(path).expect("failed to create src/target/generated.rs");

    f.write_all(PRELUDE.as_bytes()).unwrap();

    // Start generating
    generate_target_mapping(&mut f, &target_specs).unwrap();

    // Flush the data onto disk
    f.flush().unwrap();
}
