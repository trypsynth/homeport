# homeport

Finds where a desktop app keeps its config and resources. The same executable can run as a portable copy, from an Inno Setup install, from a macOS app bundle, or from a Linux AppImage, and each keeps its settings somewhere different.

## Install kinds

| Kind | Detected by | Config folder | Resources folder |
| --- | --- | --- | --- |
| `Portable` | None of the others | The executable's folder | The executable's folder |
| `Installed` | An Inno Setup `unins*.exe` next to the executable | `%APPDATA%\<name>` | The executable's folder |
| `MacBundle` | The executable is in `<name>.app/Contents/MacOS` | `~/Library/Application Support/<name>` | `Contents/Resources` |
| `AppImage` | The `APPIMAGE` environment variable | `$XDG_CONFIG_HOME/<name>` | The executable's folder |

Outside a portable copy, the config folder comes from [`dirs::config_dir`](https://docs.rs/dirs).

## Usage

```rust
use homeport::Homeport;

let home = Homeport::new("Fedra").with_config_override("FEDRA_CONFIG_DIR");
let settings = home.create_config_dir()?.join("config.json");
let sound = home.resource_dir().join("sounds/boop.mp3");
```

`with_config_override` names an environment variable that, when set, replaces the config folder. Use it for tests or to run a second copy with separate settings.

## License

MIT
