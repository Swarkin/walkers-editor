fn main() {
	if std::env::var("PROFILE").unwrap() != "release" {
		return;
	}

	/* generate licenses text */ {
		let output = std::process::Command::new("cargo")
			.args(["tree", "--format={p} - {l}"])
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
				.set_icon("assets/icon.ico")
				.compile()
				.expect("failed to register windows app icon");
		}
	}
}
