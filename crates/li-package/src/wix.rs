// Windows Installer: WiX v4 source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use li_model::Scope;

use crate::Result;
use crate::info::PackageInfo;
use crate::msix::four_part_version;
use crate::xml::{esc, guid};

#[derive(Default)]
struct Dir {
	files: Vec<(usize, String)>,
	dirs: BTreeMap<String, Dir>,
}

/// Characters Windows forbids in a folder name become spaces.
fn folder_name(s: &str) -> String {
	let t: String = s
		.chars()
		.map(|c| if "<>:\"/\\|?*".contains(c) { ' ' } else { c })
		.collect();
	t.trim().to_owned()
}

fn write_dir(
	out: &mut String,
	dir: &Dir,
	depth: usize,
	next_id: &mut usize,
	ids: &mut Vec<usize>,
	info: &PackageInfo,
	paths: &[String],
) {
	let pad = "\t".repeat(depth);
	for (i, name) in &dir.files {
		let n = *next_id;
		*next_id += 1;
		ids.push(n);
		let source = format!("$(var.SourceDir)\\{}", paths[*i].replace('/', "\\"));
		out.push_str(&format!("{pad}<Component Id=\"C{n}\" Guid=\"*\">\n"));
		let main = info.executable.as_deref() == Some(paths[*i].as_str());
		out.push_str(&format!(
			"{pad}\t<File Id=\"F{n}\" Name=\"{}\" Source=\"{}\"{}",
			esc(name),
			esc(&source),
			if info.scope == Scope::Machine {
				""
			} else {
				" KeyPath=\"no\""
			}
		));
		if main {
			out.push_str(">\n");
			out.push_str(&format!(
				"{pad}\t\t<Shortcut Id=\"S{n}\" Name=\"{}\" Directory=\"ProgramMenuFolder\" WorkingDirectory=\"INSTALLFOLDER\" Advertise=\"yes\"/>\n",
				esc(&folder_name(&info.name))
			));
			out.push_str(&format!("{pad}\t</File>\n"));
		} else {
			out.push_str("/>\n");
		}
		if info.scope != Scope::Machine {
			out.push_str(&format!(
				"{pad}\t<RegistryValue Root=\"HKCU\" Key=\"Software\\{}\\{}\\Components\" Name=\"C{n}\" Type=\"integer\" Value=\"1\" KeyPath=\"yes\"/>\n",
				esc(&folder_name(&info.publisher)),
				esc(&folder_name(&info.name))
			));
		}
		out.push_str(&format!("{pad}</Component>\n"));
	}
	for (name, sub) in &dir.dirs {
		let n = *next_id;
		*next_id += 1;
		out.push_str(&format!(
			"{pad}<Directory Id=\"D{n}\" Name=\"{}\">\n",
			esc(name)
		));
		write_dir(out, sub, depth + 1, next_id, ids, info, paths);
		out.push_str(&format!("{pad}</Directory>\n"));
	}
}

/// The `.wxs` source: one component per file under the install folder, a Start menu shortcut
/// to the main program, and a stable upgrade code derived from the product id. Compile with
/// `wix build -d SourceDir=<staging> product.wxs`.
pub fn wxs(info: &PackageInfo) -> Result<String> {
	let entries = info.payload()?;
	let paths: Vec<String> = entries.iter().map(|e| e.dest.clone()).collect();
	let mut root = Dir::default();
	for (i, e) in entries.iter().enumerate() {
		let mut parts: Vec<&str> = e.dest.split('/').collect();
		let file = parts.pop().unwrap_or_default().to_owned();
		let mut cur = &mut root;
		for p in parts {
			cur = cur.dirs.entry(p.to_owned()).or_default();
		}
		cur.files.push((i, file));
	}
	let machine = info.scope == Scope::Machine;
	let mut body = String::new();
	let mut next_id = 0usize;
	let mut ids = Vec::new();
	let depth = if machine { 4 } else { 5 };
	write_dir(
		&mut body,
		&root,
		depth,
		&mut next_id,
		&mut ids,
		info,
		&paths,
	);
	let component_refs: String = ids
		.iter()
		.map(|n| format!("\t\t\t<ComponentRef Id=\"C{n}\"/>\n"))
		.collect();
	let name = esc(&folder_name(&info.name));
	let (std_dir, open, close) = if machine {
		(
			"ProgramFiles64Folder",
			format!("\t\t\t<Directory Id=\"INSTALLFOLDER\" Name=\"{name}\">\n"),
			"\t\t\t</Directory>\n".to_owned(),
		)
	} else {
		(
			"LocalAppDataFolder",
			format!(
				"\t\t\t<Directory Id=\"ProgramsFolder\" Name=\"Programs\">\n\t\t\t\t<Directory Id=\"INSTALLFOLDER\" Name=\"{name}\">\n"
			),
			"\t\t\t\t</Directory>\n\t\t\t</Directory>\n".to_owned(),
		)
	};
	let publisher = esc(&folder_name(&info.publisher));
	Ok(format!(
		r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
	<Package Name="{name}" Manufacturer="{publisher}" Version="{version}" UpgradeCode="{upgrade}" Scope="{scope}" InstallerVersion="500" Compressed="yes">
		<MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed."/>
		<MediaTemplate EmbedCab="yes"/>
		<StandardDirectory Id="{std_dir}">
{open}{body}{close}		</StandardDirectory>
		<Feature Id="Main" Title="{name}" Level="1">
{refs}		</Feature>
	</Package>
</Wix>
"#,
		version = four_part_version(&info.version),
		upgrade = guid(&format!("liminal-installer:{}", info.id)),
		scope = if machine { "perMachine" } else { "perUser" },
		refs = component_refs,
	))
}
