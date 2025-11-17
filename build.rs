use poreader::{PoParser, PoReader, State};
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
	let id_names = read_main_language_translations(&parser, &tr_dir);
	let tr_file_names = read_translation_file_names(&tr_dir)?;

	let mut s = String::new();

	s.push_str("pub type Translation = [&'static str];\n\n");

	s.push_str("#[derive(Debug)]\n");
	s.push_str("pub enum TranslationID {");
	for id_name in &id_names {
		s.push_str("\n\t");
		s.push_str(id_name);
		s.push(',');
	}
	s.push_str("\n}\n\n");

	s.push_str("impl TranslationID {\n");
	write!(s, "\tpub const ITER: [TranslationID; {}] = [", id_names.len()).unwrap();

	for id_name in &id_names {
		s.push_str("TranslationID::");
		s.push_str(id_name);
		s.push_str(", ");
	}
	s.push_str("];\n}\n\n");

	s.push_str("#[derive(Debug, Default)]\n");
	s.push_str("pub enum TranslationLanguage {");
	s.push_str("\n\t#[default] EN,");
	for translation in &tr_file_names {
		if translation == "en" { continue; }
		s.push_str("\n\t");
		s.push_str(&translation.to_ascii_uppercase());
		s.push(',');
	}
	s.push_str("\n}\n");

	for tr_name in &tr_file_names {
		write!(s, "\npub static {}: &Translation = &[\n", tr_name.to_ascii_uppercase()).unwrap();
		let tr_file = open_translation_file(&parser, &tr_dir, &format!("{tr_name}.po"));

		for unit in tr_file.map(|x| x.unwrap()) {
			s.push_str("\t\"");
			if unit.state() == State::Final {
				let tr = unit.message().get_text().replace('\"', "\\\"");
				s.push_str(&tr);
			}
			s.push_str("\",\n");
		}

		s.push_str("];\n");
	}

	s.push_str("\npub fn get_translation(lang: TranslationLanguage) -> &'static Translation {\n");
	s.push_str("\tmatch lang {");
	for translation in &tr_file_names {
		s.push_str("\n\t\tTranslationLanguage::");
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

fn read_main_language_translations(parser: &PoParser, translations_dir: &Path) -> Vec<String> {
	let main_language_file = open_translation_file(parser, translations_dir, MAIN_LANGUAGE_FILE);
	let mut id_names = vec![];

	for unit in main_language_file.map(|x| x.unwrap()) {
		id_names.push(unit.message().get_id().to_ascii_uppercase());
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
