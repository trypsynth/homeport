//! Where a desktop app keeps its config and its resources.
//!
//! The same executable can run from an install, from an unzipped portable folder, from a macOS
//! app bundle, or from a Linux `AppImage`, and each keeps its settings somewhere different. A
//! portable copy keeps them beside the executable so the whole folder can move between machines.
//! The others can't write there, so they use the operating system's config folder, as
//! [`dirs::config_dir`] reports it.
//!
//! ```no_run
//! let home = homeport::Homeport::new("Fedra");
//! let settings = home.config_dir().join("config.json");
//! let sound = home.resource_dir().join("sounds/boop.mp3");
//! ```

use std::{
	env, fs,
	path::{Path, PathBuf},
};

/// How the running app was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
	/// Unpacked from an archive. Its settings live beside the executable.
	Portable,
	/// Installed by an Inno Setup installer, detected by the `unins*.exe` uninstaller it leaves
	/// beside the executable.
	Installed,
	/// Running from a macOS `.app` bundle. Writing inside the bundle would break its signature.
	MacBundle,
	/// Running from a Linux `AppImage`, which mounts itself read-only at a new path each launch.
	AppImage,
}

impl InstallKind {
	/// Whether settings belong beside the executable rather than in the user's config folder.
	#[must_use]
	pub const fn is_portable(self) -> bool {
		matches!(self, Self::Portable)
	}
}

/// Where the running app lives, and where its config and resources go.
#[derive(Debug, Clone)]
pub struct Homeport {
	name: String,
	exe_dir: PathBuf,
	kind: InstallKind,
	config_override: Option<String>,
}

impl Homeport {
	/// Detects how the running executable was installed.
	///
	/// `name` is the folder the app's config goes in when it isn't portable, such as `Fedra`.
	#[must_use]
	pub fn new(name: impl Into<String>) -> Self {
		let exe = env::current_exe().unwrap_or_default();
		Self::detect(name.into(), &exe, env::var_os("APPIMAGE").is_some())
	}

	fn detect(name: String, exe: &Path, in_appimage: bool) -> Self {
		let exe_dir = exe.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
		let kind = if in_appimage {
			InstallKind::AppImage
		} else if is_mac_bundle(&exe_dir) {
			InstallKind::MacBundle
		} else if has_uninstaller(&exe_dir) {
			InstallKind::Installed
		} else {
			InstallKind::Portable
		};
		Self { name, exe_dir, kind, config_override: None }
	}

	/// Lets the environment variable `var` choose the config folder, overriding the install kind.
	/// This is useful for tests, and for running a second copy with separate settings.
	#[must_use]
	pub fn with_config_override(mut self, var: impl Into<String>) -> Self {
		self.config_override = Some(var.into());
		self
	}

	#[must_use]
	pub const fn kind(&self) -> InstallKind {
		self.kind
	}

	/// The folder the executable is in.
	#[must_use]
	pub fn exe_dir(&self) -> &Path {
		&self.exe_dir
	}

	/// Where the app's settings go: the folder named by the config override variable if it's set,
	/// the executable's folder for a portable copy, and otherwise the app's folder in the user's
	/// config folder, such as `%APPDATA%\Fedra`.
	///
	/// The folder might not exist yet. Use [`create_config_dir`](Self::create_config_dir) to
	/// create it.
	#[must_use]
	pub fn config_dir(&self) -> PathBuf {
		if let Some(dir) = self.config_override.as_ref().and_then(env::var_os).filter(|dir| !dir.is_empty()) {
			return PathBuf::from(dir);
		}
		if self.kind.is_portable() {
			return self.exe_dir.clone();
		}
		dirs::config_dir().map_or_else(|| self.exe_dir.clone(), |dir| dir.join(&self.name))
	}

	/// [`config_dir`](Self::config_dir), created if it doesn't exist.
	///
	/// # Errors
	///
	/// Returns an error if the folder can't be created.
	pub fn create_config_dir(&self) -> std::io::Result<PathBuf> {
		let dir = self.config_dir();
		fs::create_dir_all(&dir)?;
		Ok(dir)
	}

	/// Where the files the app ships with are, such as sounds and the readme: `Contents/Resources`
	/// in a macOS app bundle, and otherwise the executable's folder.
	#[must_use]
	pub fn resource_dir(&self) -> PathBuf {
		match self.kind {
			InstallKind::MacBundle => self.exe_dir.with_file_name("Resources"),
			_ => self.exe_dir.clone(),
		}
	}
}

/// Whether `exe_dir` is `Contents/MacOS` inside a `.app` folder.
fn is_mac_bundle(exe_dir: &Path) -> bool {
	exe_dir.ends_with("Contents/MacOS")
		&& exe_dir
			.parent()
			.and_then(Path::parent)
			.and_then(Path::extension)
			.is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
}

/// Whether `dir` holds an Inno Setup uninstaller. It's `unins000.exe` unless another app was
/// installed into the same folder, when the number goes up.
fn has_uninstaller(dir: &Path) -> bool {
	let Ok(entries) = fs::read_dir(dir) else { return false };
	entries.flatten().any(|entry| {
		let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
		name.strip_prefix("unins")
			.and_then(|rest| rest.strip_suffix(".exe"))
			.is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
			&& entry.file_type().is_ok_and(|kind| kind.is_file())
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	struct TempDir(PathBuf);

	impl TempDir {
		fn new(label: &str) -> Self {
			let dir = env::temp_dir().join(format!("homeport-{label}-{}", std::process::id()));
			let _ = fs::remove_dir_all(&dir);
			fs::create_dir_all(&dir).unwrap();
			Self(dir)
		}
	}

	impl Drop for TempDir {
		fn drop(&mut self) {
			let _ = fs::remove_dir_all(&self.0);
		}
	}

	fn detect(exe: &Path) -> Homeport {
		Homeport::detect("Test App".to_string(), exe, false)
	}

	#[test]
	fn portable_keeps_config_beside_the_executable() {
		let dir = TempDir::new("portable");
		let home = detect(&dir.0.join("app.exe"));
		assert_eq!(home.kind(), InstallKind::Portable);
		assert_eq!(home.config_dir(), dir.0);
		assert_eq!(home.resource_dir(), dir.0);
	}

	#[test]
	fn inno_uninstaller_means_installed() {
		let dir = TempDir::new("installed");
		fs::write(dir.0.join("unins001.exe"), "").unwrap();
		let home = detect(&dir.0.join("app.exe"));
		assert_eq!(home.kind(), InstallKind::Installed);
		assert_eq!(home.config_dir(), dirs::config_dir().unwrap().join("Test App"));
		assert_eq!(home.resource_dir(), dir.0);
	}

	#[test]
	fn lookalike_files_are_not_uninstallers() {
		let dir = TempDir::new("lookalike");
		for name in ["uninstall.exe", "unins.exe", "unins000.dat"] {
			fs::write(dir.0.join(name), "").unwrap();
		}
		fs::create_dir(dir.0.join("unins002.exe")).unwrap();
		assert_eq!(detect(&dir.0.join("app.exe")).kind(), InstallKind::Portable);
	}

	#[test]
	fn mac_bundle_uses_contents_resources() {
		let exe = Path::new("/Applications/Fedra.app/Contents/MacOS/fedra");
		let home = detect(exe);
		assert_eq!(home.kind(), InstallKind::MacBundle);
		assert_eq!(home.resource_dir(), Path::new("/Applications/Fedra.app/Contents/Resources"));
		assert_ne!(home.config_dir(), home.exe_dir());
		assert_eq!(detect(Path::new("/tmp/Contents/MacOS/fedra")).kind(), InstallKind::Portable);
	}

	#[test]
	fn appimage_wins() {
		let exe = Path::new("/tmp/.mount_FedraX/usr/bin/fedra");
		let home = Homeport::detect("Test App".to_string(), exe, true);
		assert_eq!(home.kind(), InstallKind::AppImage);
		assert_eq!(home.config_dir(), dirs::config_dir().unwrap().join("Test App"));
	}

	#[test]
	fn override_variable_wins() {
		let dir = TempDir::new("override");
		let var = "HOMEPORT_TEST_CONFIG_DIR";
		let home = detect(&dir.0.join("app.exe")).with_config_override(var);
		// SAFETY: no other test reads or writes this variable.
		unsafe { env::set_var(var, "/somewhere/else") };
		assert_eq!(home.config_dir(), Path::new("/somewhere/else"));
		unsafe { env::set_var(var, "") };
		assert_eq!(home.config_dir(), dir.0, "an empty override is ignored");
		unsafe { env::remove_var(var) };
	}
}
