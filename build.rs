use poreader::{PoParser, PoReader};
use std::fmt::Write;
use std::fs::File;
use std::path::{Path, PathBuf};

const MAIN_LANGUAGE_FILE: &str = "en.po";

fn main() {
	println!("cargo:rerun-if-changed=src/translations/");

	assert!(
		!(cfg!(feature = "renderer_glow") && (cfg!(feature = "renderer_wgpu_dx12") || cfg!(feature = "renderer_wgpu_vulkan") || cfg!(feature = "renderer_wgpu_gles"))),
		"only one of the renderers must be enabled at the same time"
	);

	assert!(
		cfg!(any(feature = "renderer_glow", feature = "renderer_wgpu_dx12", feature = "renderer_wgpu_vulkan", feature = "renderer_wgpu_gles")),
		"one of the renderers must be enabled"
	);

	generate_translations().unwrap();

	if std::env::var("PROFILE").unwrap() != "release" {
		return;
	}

	/* generate licenses text */ {
		let output = std::process::Command::new("cargo")
			.args(["tree", "--format={p} - {l}", "--charset=ascii"])
			.output()
			.expect("failed to run command");

		assert!(output.status.success(), "failed to run command");

		let newline_index = output.stdout
			.iter()
			.position(|&b| b == b'\n')
			.expect("failed to find newline in command output");

		let mut text = String::from("┌");
		let content = str::from_utf8(&output.stdout[newline_index + 4..])
			.expect("failed to parse utf8");

		text.push_str(content);

		let out_dir = std::env::var("OUT_DIR")
			.expect("failed to get environment variable");

		std::fs::write(format!("{out_dir}/deps.txt"), text)
			.expect("failed to write file");
	}

	/* link windows app icon */ {
		let target_os = std::env::var("CARGO_CFG_TARGET_OS")
			.expect("failed to get environment variable");

		if target_os == "windows" {
			winresource::WindowsResource::new()
				.set_icon("assets/favicon.ico")
				.compile()
				.expect("failed to register windows app icon");
		}
	}
}

fn generate_translations() -> std::io::Result<()> {
	println!("cargo:rerun-if-changed=src/translations/");

	let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
	let tr_dir = manifest_dir.join("translations");

	let parser = PoParser::new();
	let main_lang_file = open_translation_file(&parser, &tr_dir, MAIN_LANGUAGE_FILE);
	let main_lang_id_names = read_translation_file_entries(main_lang_file);

	let tr_file_names = read_translation_file_names(&tr_dir)?;

	let mut s = String::new();

	s.push_str("pub type Translation = [&'static str];\n\n");

	s.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\n");
	s.push_str("pub enum TranslationID {");
	for (key, _) in &main_lang_id_names {
		s.push_str("\n\t");

		let mut chars = key.chars();
		let first = chars.next().unwrap();
		s.push(first.to_ascii_uppercase());
		s.push_str(chars.as_str());

		s.push(',');
	}
	s.push_str("\n}\n\n");

	s.push_str("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\n");
	s.push_str("pub enum Language {");
	for tr_name in &tr_file_names {
		s.push_str("\n\t");
		if tr_name == "en" { s.push_str("#[default] "); }
		s.push_str(&tr_name.to_ascii_uppercase());
		s.push(',');
	}
	s.push_str("\n}\n\n");

	s.push_str("impl Language {\n");
	write!(s, "\tpub const ITER: [Self; {}] = [", tr_file_names.len()).unwrap();
	for tr_name in &tr_file_names {
		s.push_str("Self::");
		s.push_str(&tr_name.to_ascii_uppercase());
		s.push_str(", ");
	}
	s.push_str("];\n}\n\n");

	s.push_str("pub static EN: &Translation = &[\n");
	for (_, text) in &main_lang_id_names {
		s.push_str("\t\"");
		s.push_str(&text.replace('\"', "\\\""));
		s.push_str("\",\n");
	}
	s.push_str("];\n");

	for tr_name in &tr_file_names {
		if tr_name == "en" { continue; }

		write!(s, "\npub static {}: &Translation = &[\n", tr_name.to_ascii_uppercase()).unwrap();
		let tr_file = open_translation_file(&parser, &tr_dir, &format!("{tr_name}.po"));

		let mut words = main_lang_id_names.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();

		for unit in tr_file.map(|x| x.unwrap()) {
			if unit.state() != poreader::State::Final { continue; }

			let i = main_lang_id_names.iter().position(|(k, _)| k == unit.message().get_id()).unwrap();
			let tr = unit.message().get_text().replace('\"', "\\\"");
			words[i] = tr;
		}

		for word in words {
			s.push_str("\t\"");
			s.push_str(&word);
			s.push_str("\",\n");
		}

		s.push_str("];\n");
	}

	s.push_str("\npub fn get_translation(lang: Language) -> &'static Translation {\n");
	s.push_str("\tmatch lang {");
	for translation in &tr_file_names {
		s.push_str("\n\t\tLanguage::");
		s.push_str(&translation.to_ascii_uppercase());
		s.push_str(" => ");
		s.push_str(&translation.to_ascii_uppercase());
		s.push(',');
	}
	s.push_str("\n\t}\n}\n");

	std::fs::write(out_dir.join("translations.rs"), s)?;
	Ok(())
}

fn open_translation_file<'a>(parser: &'a PoParser, translations_dir: &Path, name: &str) -> PoReader<'a, File> {
	let file = File::open(translations_dir.join(name)).unwrap();
	parser.parse(file).unwrap()
}

fn read_translation_file_entries(language_file: PoReader<File>) -> Vec<(String, String)> {
	let mut id_names = vec![];

	for unit in language_file.map(|x| x.unwrap()) {
		id_names.push((unit.message().get_id().to_owned(), unit.message().get_text().to_owned()));
	}

	id_names
}

fn read_translation_file_names(translations_dir: &Path) -> std::io::Result<Vec<String>> {
	let mut names = Vec::new();

	for entry in std::fs::read_dir(translations_dir)? {
		let entry = entry?;
		let file_name = entry.file_name().into_string().unwrap();
		if let Some((name, ext)) = file_name.rsplit_once('.') && ext == "po" {
			names.push(name.to_owned());
		}
	}

	Ok(names)
}
