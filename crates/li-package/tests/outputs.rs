// Golden-file and behaviour tests for the output generators.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::process::Command;

use li_model::{ComponentsFile, InstallerFile, Scope};
use li_package::{
	Arch, Entry, PackageInfo, arch, deb, freebsd, macos, msix, resolve_version, rpm, run, wix,
};

fn fixture(scope: Scope) -> PackageInfo {
	PackageInfo {
		id: "hq.liminal.lt-fizz".into(),
		name: "Lieutenant Fizz — Episode One".into(),
		publisher: "Liminal HQ".into(),
		version: "1.0.0".into(),
		description: "A space adventure about carbonated heroics.".into(),
		licence: Some("MIT".into()),
		homepage: Some("https://example.org/fizz".into()),
		arch: Arch::X86_64,
		scope,
		target: "linux-x86_64".into(),
		receipt_target: "linux-x86_64:native".into(),
		components: vec!["game".into(), "sound".into()],
		files: vec![
			Entry::bytes("bin/fizz", b"#!/bin/sh\necho fizz\n", true),
			Entry::bytes("SOUND/SB.DRV", b"driver", false),
		],
		executable: Some("bin/fizz".into()),
		engine: None,
	}
}

fn golden(name: &str, actual: &str) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden")
		.join(format!("{name}.golden"));
	if std::env::var_os("UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, actual).unwrap();
		return;
	}
	let expected = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing golden file {}", path.display()));
	assert_eq!(expected, actual, "golden mismatch for {name}");
}

#[test]
fn deb_control() {
	golden("control", &deb::control(&fixture(Scope::Machine), 4));
}

#[test]
fn rpm_spec() {
	golden("fizz.spec", &rpm::spec(&fixture(Scope::Machine)));
}

#[test]
fn arch_pkgbuild() {
	let info = fixture(Scope::Machine);
	let (_, sum) = arch::source_tarball(&info).unwrap();
	golden("PKGBUILD", &arch::pkgbuild(&info, &sum));
}

#[test]
fn source_tarballs_are_reproducible() {
	let info = fixture(Scope::Machine);
	assert_eq!(
		rpm::source_tarball(&info).unwrap(),
		rpm::source_tarball(&info).unwrap()
	);
}

#[test]
fn wix_machine_and_user() {
	golden("machine.wxs", &wix::wxs(&fixture(Scope::Machine)).unwrap());
	golden("user.wxs", &wix::wxs(&fixture(Scope::User)).unwrap());
}

#[test]
fn msix_manifest() {
	let mut info = fixture(Scope::User);
	info.executable = Some("bin/fizz.exe".into());
	golden("AppxManifest.xml", &msix::manifest(&info));
	assert_eq!(msix::four_part_version("1.2.3-beta.1"), "1.2.3.0");
	assert_eq!(msix::four_part_version("2"), "2.0.0.0");
}

#[test]
fn macos_files() {
	let info = fixture(Scope::Machine);
	golden("distribution.xml", &macos::distribution_xml(&info));
	golden("Info.plist", &macos::info_plist(&info));
}

#[test]
fn freebsd_manifest() {
	golden(
		"+MANIFEST",
		&freebsd::manifest(&fixture(Scope::Machine), 14).unwrap(),
	);
}

#[test]
fn run_stub() {
	let info = fixture(Scope::User);
	golden("stub.sh", &run::stub(&info, "deadbeef"));
}

#[test]
fn run_file_extracts_lists_and_checks() {
	let dir = tempfile::tempdir().unwrap();
	let out = dir.path().join("fizz.run");
	run::build(&fixture(Scope::User), &out).unwrap();
	let prefix = dir.path().join("prefix");
	let sh = |args: &[&str]| {
		Command::new("sh")
			.arg(&out)
			.args(args)
			.output()
			.expect("sh runs")
	};
	let list = sh(&["--list"]);
	assert!(list.status.success());
	let listing = String::from_utf8_lossy(&list.stdout);
	assert!(listing.contains("bin/fizz") && listing.contains(".liminal/receipt.json"));
	let check = sh(&["--check"]);
	assert!(
		check.status.success(),
		"{}",
		String::from_utf8_lossy(&check.stderr)
	);
	let install = sh(&["--prefix", prefix.to_str().unwrap()]);
	assert!(
		install.status.success(),
		"{}",
		String::from_utf8_lossy(&install.stderr)
	);
	assert_eq!(
		std::fs::read(prefix.join("SOUND/SB.DRV")).unwrap(),
		b"driver"
	);
	let receipt: li_model::Receipt =
		serde_json::from_slice(&std::fs::read(prefix.join(".liminal/receipt.json")).unwrap())
			.unwrap();
	assert_eq!(receipt.product, "hq.liminal.lt-fizz");
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let mode = std::fs::metadata(prefix.join("bin/fizz"))
			.unwrap()
			.permissions()
			.mode();
		assert_eq!(mode & 0o111, 0o111);
	}
}

#[test]
fn run_file_detects_a_damaged_payload() {
	let dir = tempfile::tempdir().unwrap();
	let out = dir.path().join("fizz.run");
	run::build(&fixture(Scope::User), &out).unwrap();
	let mut bytes = std::fs::read(&out).unwrap();
	let last = bytes.len() - 20;
	bytes[last] ^= 0xff;
	std::fs::write(&out, bytes).unwrap();
	let o = Command::new("sh")
		.arg(&out)
		.arg("--check")
		.output()
		.unwrap();
	assert!(!o.status.success());
}

#[cfg(unix)]
#[test]
fn run_file_hands_over_to_the_engine() {
	let mut info = fixture(Scope::User);
	info.files.push(Entry::bytes(
		"engine",
		b"#!/bin/sh\necho \"engine $*\" > \"$3/engine.out\"\n",
		true,
	));
	info.engine = Some("engine".into());
	let dir = tempfile::tempdir().unwrap();
	let out = dir.path().join("fizz.run");
	run::build(&info, &out).unwrap();
	let prefix = dir.path().join("p");
	let o = Command::new("sh")
		.arg(&out)
		.arg("--prefix")
		.arg(&prefix)
		.output()
		.unwrap();
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let said = std::fs::read_to_string(prefix.join("engine.out")).unwrap();
	assert!(said.starts_with("engine install --from"));
}

#[test]
fn deb_tree_layout() {
	let dir = tempfile::tempdir().unwrap();
	let size = deb::write_tree(&fixture(Scope::Machine), dir.path()).unwrap();
	assert_eq!(size, 1);
	assert!(dir.path().join("DEBIAN/control").is_file());
	assert!(dir.path().join("opt/lt-fizz/bin/fizz").is_file());
	assert!(
		dir.path()
			.join("opt/lt-fizz/.liminal/receipt.json")
			.is_file()
	);
	#[cfg(unix)]
	assert_eq!(
		std::fs::read_link(dir.path().join("usr/bin/lt-fizz")).unwrap(),
		Path::new("/opt/lt-fizz/bin/fizz")
	);
}

#[test]
fn from_project_expands_globs_and_placeholders() {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path();
	std::fs::create_dir_all(root.join("dist/linux-x86_64")).unwrap();
	std::fs::create_dir_all(root.join("dist/sound")).unwrap();
	std::fs::write(root.join("dist/linux-x86_64/fizz"), b"exe").unwrap();
	std::fs::write(root.join("dist/sound/A.DRV"), b"a").unwrap();
	std::fs::write(root.join("dist/sound/B.DRV"), b"b").unwrap();
	std::fs::write(root.join("dist/sound/README"), b"r").unwrap();
	let installer: InstallerFile = toml::from_str(
		r#"
		[product]
		id = "hq.liminal.lt-fizz"
		name = "Fizz"
		publisher = "Liminal HQ"
		version = { from = "git-tag" }
		[[target]]
		id = "linux-x86_64"
		renderer = "tui"
		"#,
	)
	.unwrap();
	let components: ComponentsFile = toml::from_str(
		r#"
		[[component]]
		id = "game"
		name = "Game"
		required = true
		files = [{ from = "dist/{target}/fizz", to = "{install}/bin/" }]
		shortcut = [{ name = "Fizz", target = "{install}/bin/fizz", in = "start_menu" }]
		[[component]]
		id = "sound"
		name = "Sound"
		files = [{ from = "dist/sound/*.DRV", to = "{install}/SOUND/" }]
		[[component]]
		id = "dos-only"
		name = "DOS"
		targets = ["dos-x86"]
		files = [{ from = "missing", to = "{install}/" }]
		"#,
	)
	.unwrap();
	let target = &installer.targets[0];
	let version = resolve_version(&installer.product.version, Some("v2.1.0")).unwrap();
	assert_eq!(version, "2.1.0");
	assert!(resolve_version(&installer.product.version, None).is_err());
	let info = PackageInfo::from_project(&installer, &components, target, &version, root).unwrap();
	let dests: Vec<_> = info.files.iter().map(|e| e.dest.as_str()).collect();
	assert_eq!(dests, ["SOUND/A.DRV", "SOUND/B.DRV", "bin/fizz"]);
	assert_eq!(info.components, ["game", "sound"]);
	assert_eq!(info.executable.as_deref(), Some("bin/fizz"));
	assert_eq!(info.slug(), "lt-fizz");
}

#[test]
fn from_project_reports_missing_files_and_foreign_placeholders() {
	let dir = tempfile::tempdir().unwrap();
	let installer: InstallerFile = toml::from_str(
		"[product]\nid='a.b'\nname='A'\npublisher='P'\nversion='1'\n[[target]]\nid='linux-x86_64'\nrenderer='tui'\n",
	)
	.unwrap();
	let missing: ComponentsFile =
		toml::from_str("[[component]]\nid='c'\nname='C'\nfiles=[{from='nope', to='{install}/'}]\n")
			.unwrap();
	let err =
		PackageInfo::from_project(&installer, &missing, &installer.targets[0], "1", dir.path())
			.unwrap_err();
	assert!(err.to_string().contains("matches no file"), "{err}");
	std::fs::write(dir.path().join("f"), b"x").unwrap();
	let foreign: ComponentsFile =
		toml::from_str("[[component]]\nid='c'\nname='C'\nfiles=[{from='f', to='{start_menu}/'}]\n")
			.unwrap();
	let err =
		PackageInfo::from_project(&installer, &foreign, &installer.targets[0], "1", dir.path())
			.unwrap_err();
	assert!(err.to_string().contains("{start_menu}"), "{err}");
}
