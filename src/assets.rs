//! Asset loading: Lua script, sprite sheet, and SFX.
//!
//! The free functions (`load_script`, `load_sprites`, `scan_sfx`, `load_sfx`)
//! are the low-level primitives. `SfxLibrary` and `SpriteSheet` wrap them
//! with the "reload-if-changed" invariant used by both `session::run` and
//! `tools::run`.

use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

/// Reads the script file and executes it on the given Lua VM, redefining
/// the `_init` / `_update` / `_draw` globals. Used for both initial load
/// and live reload.
pub fn load_script(lua: &Lua, path: &str) -> LuaResult<()> {
    let source = std::fs::read_to_string(path).map_err(LuaError::external)?;
    lua.load(&source).set_name(path).exec()
}

/// Tries to load the sprite sheet (sprites.png next to the script). Returns
/// None on any failure. Missing file is not an error; a decode failure
/// prints to stderr.
pub fn load_sprites(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    path: &Path,
) -> Option<Texture2D> {
    if !path.exists() {
        return None;
    }
    let path_str = path.to_str()?;
    match rl.load_texture(thread, path_str) {
        Ok(tex) => Some(tex),
        Err(e) => {
            eprintln!("[usagi] failed to load sprites {}: {}", path.display(), e);
            None
        }
    }
}

/// Scans `<dir>` for .wav files and returns a manifest of stem to mtime.
/// Used to detect when sfx need reloading (file added, removed, or edited).
pub fn scan_sfx(dir: &Path) -> HashMap<String, SystemTime> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        out.insert(stem.to_string(), mtime);
    }
    out
}

/// Loads all .wav files in `<dir>` into a name-to-Sound map, keyed by file
/// stem (e.g. `sfx/jump.wav` -> "jump"). Individual decode failures log to
/// stderr; the rest still load.
/// Owns the loaded sounds + a manifest of their mtimes. `reload_if_changed`
/// re-scans the directory and reloads everything when any file changed,
/// was added, or removed. The lifetime is tied to the `RaylibAudio`.
pub struct SfxLibrary<'a> {
    pub sounds: HashMap<String, Sound<'a>>,
    manifest: HashMap<String, SystemTime>,
}

impl<'a> SfxLibrary<'a> {
    pub fn empty() -> Self {
        Self {
            sounds: HashMap::new(),
            manifest: HashMap::new(),
        }
    }

    pub fn load(audio: &'a RaylibAudio, dir: &Path) -> Self {
        Self {
            sounds: load_sfx(audio, dir),
            manifest: scan_sfx(dir),
        }
    }

    /// Returns true if the library was reloaded this call.
    pub fn reload_if_changed(&mut self, audio: &'a RaylibAudio, dir: &Path) -> bool {
        let new_manifest = scan_sfx(dir);
        if new_manifest == self.manifest {
            return false;
        }
        self.manifest = new_manifest;
        self.sounds = load_sfx(audio, dir);
        true
    }

    pub fn play(&self, name: &str) {
        if let Some(sound) = self.sounds.get(name) {
            sound.play();
        }
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }
}

/// Owns the sprite sheet texture + its mtime. `reload_if_changed` re-reads
/// the file if it has been modified since the last load.
pub struct SpriteSheet {
    pub texture: Option<Texture2D>,
    mtime: Option<SystemTime>,
}

impl SpriteSheet {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, path: &Path) -> Self {
        Self {
            texture: load_sprites(rl, thread, path),
            mtime: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
        }
    }

    /// Returns true if the sheet was reloaded this call.
    pub fn reload_if_changed(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        path: &Path,
    ) -> bool {
        let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else {
            return false;
        };
        if Some(modified) == self.mtime {
            return false;
        }
        self.mtime = Some(modified);
        self.texture = load_sprites(rl, thread, path);
        true
    }

    pub fn texture(&self) -> Option<&Texture2D> {
        self.texture.as_ref()
    }
}

pub fn load_sfx<'a>(audio: &'a RaylibAudio, dir: &Path) -> HashMap<String, Sound<'a>> {
    let mut sounds = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sounds;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let (Some(stem), Some(path_str)) =
            (path.file_stem().and_then(|s| s.to_str()), path.to_str())
        else {
            continue;
        };
        match audio.new_sound(path_str) {
            Ok(sound) => {
                sounds.insert(stem.to_string(), sound);
            }
            Err(e) => eprintln!("[usagi] failed to load sfx {}: {}", path.display(), e),
        }
    }
    sounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn scan_sfx_finds_wav_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("jump.wav"), b"fake").unwrap();
        fs::write(dir.path().join("coin.wav"), b"fake").unwrap();
        let manifest = scan_sfx(dir.path());
        assert!(manifest.contains_key("jump"));
        assert!(manifest.contains_key("coin"));
        assert_eq!(manifest.len(), 2);
    }

    #[test]
    fn scan_sfx_ignores_non_wav() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("jump.wav"), b"fake").unwrap();
        fs::write(dir.path().join("readme.txt"), b"hi").unwrap();
        fs::write(dir.path().join("bgm.ogg"), b"fake").unwrap();
        let manifest = scan_sfx(dir.path());
        assert_eq!(manifest.len(), 1);
        assert!(manifest.contains_key("jump"));
    }

    #[test]
    fn scan_sfx_missing_dir_returns_empty() {
        let manifest = scan_sfx(Path::new("/does/not/exist/at/all"));
        assert!(manifest.is_empty());
    }

    #[test]
    fn load_script_executes_and_sets_globals() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("t.lua");
        fs::write(&path, "x = 42\nfunction _init() y = 99 end").unwrap();

        load_script(&lua, path.to_str().unwrap()).unwrap();
        let x: i32 = lua.globals().get("x").unwrap();
        assert_eq!(x, 42);
        let init: LuaFunction = lua.globals().get("_init").unwrap();
        init.call::<()>(()).unwrap();
        let y: i32 = lua.globals().get("y").unwrap();
        assert_eq!(y, 99);
    }

    #[test]
    fn load_script_returns_err_on_syntax_error() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("broken.lua");
        fs::write(&path, "function _update(dt)").unwrap(); // missing end
        assert!(load_script(&lua, path.to_str().unwrap()).is_err());
    }

    #[test]
    fn load_script_returns_err_on_missing_file() {
        let lua = Lua::new();
        assert!(load_script(&lua, "/does/not/exist.lua").is_err());
    }

    /// Every `.lua` in `examples/` (including `<subdir>/main.lua`) must at
    /// least parse. Catches broken examples before `just example X` does.
    #[test]
    fn every_example_script_parses() {
        let lua = Lua::new();
        let examples_dir = Path::new("examples");
        assert!(
            examples_dir.is_dir(),
            "examples/ missing; test must run from repo root"
        );
        for entry in fs::read_dir(examples_dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                let main = path.join("main.lua");
                if main.is_file() {
                    parse_ok(&lua, &main);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                parse_ok(&lua, &path);
            }
        }
    }

    fn parse_ok(lua: &Lua, path: &Path) {
        let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        lua.load(&src)
            .set_name(path.to_str().unwrap())
            .into_function()
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
    }
}
