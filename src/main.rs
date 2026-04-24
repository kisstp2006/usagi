use mlua::prelude::*;
use sola_raylib::prelude::*;

const GAME_WIDTH: f32 = 320.;
const GAME_HEIGHT: f32 = 180.;

/// draws the game's render target to the screen, scaled
fn draw_render_target(
    d: &mut RaylibDrawHandle,
    rt: &mut RenderTexture2D,
    screen_w: i32,
    screen_h: i32,
    pixel_perfect: bool,
) {
    let game_w = GAME_WIDTH;
    let game_h = GAME_HEIGHT;
    let mut scale = (screen_w as f32 / game_w).min(screen_h as f32 / game_h);
    if pixel_perfect {
        scale = scale.floor();
    }
    if scale < 1.0 {
        scale = 1.0;
    }
    let scaled_w = game_w * scale;
    let scaled_h = game_h * scale;
    let dest_rect = Rectangle {
        x: (screen_w / 2) as f32,
        y: (screen_h / 2) as f32,
        width: scaled_w,
        height: scaled_h,
    };
    let origin = Vector2::new(scaled_w / 2.0, scaled_h / 2.0);

    d.draw_texture_pro(
        rt.texture(),
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: game_w,
            height: -game_h,
        },
        dest_rect,
        origin,
        0.,
        Color::WHITE,
    );
}

/// Maps a palette index (0–15) to an RGBA color. Values outside the palette
/// return magenta as an obvious "unknown color" sentinel.
///
/// Palette is Pico-8's — see `setup_api` for the `gfx.COLOR_*` names.
fn palette(c: i32) -> Color {
    match c {
        0 => Color::new(0, 0, 0, 255),        // black
        1 => Color::new(29, 43, 83, 255),     // dark blue
        2 => Color::new(126, 37, 83, 255),    // dark purple
        3 => Color::new(0, 135, 81, 255),     // dark green
        4 => Color::new(171, 82, 54, 255),    // brown
        5 => Color::new(95, 87, 79, 255),     // dark gray
        6 => Color::new(194, 195, 199, 255),  // light gray
        7 => Color::new(255, 241, 232, 255),  // white
        8 => Color::new(255, 0, 77, 255),     // red
        9 => Color::new(255, 163, 0, 255),    // orange
        10 => Color::new(255, 236, 39, 255),  // yellow
        11 => Color::new(0, 228, 54, 255),    // green
        12 => Color::new(41, 173, 255, 255),  // blue
        13 => Color::new(131, 118, 156, 255), // indigo
        14 => Color::new(255, 119, 168, 255), // pink
        15 => Color::new(255, 204, 170, 255), // peach
        _ => Color::new(255, 0, 255, 255),    // magenta (unknown)
    }
}

/// converts the u32 into the `KeyboardKey` enum
fn key_from_u32(k: u32) -> Option<KeyboardKey> {
    use KeyboardKey::*;
    match k {
        x if x == KEY_LEFT as u32 => Some(KEY_LEFT),
        x if x == KEY_RIGHT as u32 => Some(KEY_RIGHT),
        x if x == KEY_UP as u32 => Some(KEY_UP),
        x if x == KEY_DOWN as u32 => Some(KEY_DOWN),
        x if x == KEY_Z as u32 => Some(KEY_Z),
        x if x == KEY_X as u32 => Some(KEY_X),
        _ => None,
    }
}

/// Reads the script file and executes it on the given Lua VM, redefining
/// the `_init` / `_update` / `_draw` globals. Used for both initial load
/// and live reload.
fn load_script(lua: &Lua, path: &str) -> LuaResult<()> {
    let source = std::fs::read_to_string(path).map_err(LuaError::external)?;
    lua.load(&source).set_name(path).exec()
}

/// Resolves the CLI arg to a concrete script file. Accepts any of:
///   - path to a `.lua` file
///   - path to a directory containing `main.lua`
///   - path without extension that has a sibling `.lua` file
///
/// Errors with a helpful message if none match.
fn resolve_script_path(arg: &str) -> Result<String, String> {
    let path = std::path::Path::new(arg);
    if path.is_dir() {
        let main = path.join("main.lua");
        if main.exists() {
            return main
                .to_str()
                .map(String::from)
                .ok_or_else(|| format!("non-utf8 path: {}", main.display()));
        }
        return Err(format!(
            "no main.lua found in directory '{}'. Create a main.lua there, or pass a .lua file directly.",
            path.display()
        ));
    }
    if path.is_file() {
        return Ok(arg.to_string());
    }
    let with_lua = path.with_extension("lua");
    if with_lua.is_file() {
        return with_lua
            .to_str()
            .map(String::from)
            .ok_or_else(|| format!("non-utf8 path: {}", with_lua.display()));
    }
    Err(format!(
        "script not found: '{}'. Pass a .lua file, a directory with main.lua, or a name with a sibling .lua.",
        arg
    ))
}

/// Tries to load the sprite sheet (sprites.png next to the script). Returns
/// None on any failure — missing file is not an error, a decode failure
/// prints to stderr.
fn load_sprites(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    path: &std::path::Path,
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

/// Scans `<dir>` for .wav files and returns a manifest of stem → mtime.
/// Used to detect when sfx need reloading (file added, removed, or edited).
fn scan_sfx(dir: &std::path::Path) -> std::collections::HashMap<String, std::time::SystemTime> {
    let mut out = std::collections::HashMap::new();
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

/// Loads all .wav files in `<dir>` into a name → Sound map, keyed by file
/// stem (e.g. `sfx/jump.wav` → "jump"). Individual decode failures log to
/// stderr; the rest still load.
fn load_sfx<'a>(
    audio: &'a RaylibAudio,
    dir: &std::path::Path,
) -> std::collections::HashMap<String, Sound<'a>> {
    let mut sounds = std::collections::HashMap::new();
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

/// Records a Lua error: prints to stderr and stores the message so it can
/// be displayed on-screen. Wraps every call into user Lua so a typo /
/// nil-call / runtime error doesn't tear down the process.
fn record_err(state: &mut Option<String>, label: &str, result: LuaResult<()>) {
    if let Err(e) = result {
        let msg = format!("{}: {}", label, e);
        eprintln!("[usagi] {}", msg);
        *state = Some(msg);
    }
}

/// Draws a full-width error banner at the bottom of the window. Shown only
/// when user Lua has errored; cleared on successful reload or F5 reset.
fn draw_error_overlay(d: &mut RaylibDrawHandle, err: &str, screen_w: i32, screen_h: i32) {
    const PADDING: i32 = 12;
    const TITLE_SIZE: i32 = 20;
    const MSG_SIZE: i32 = 16;
    const LINE_H: i32 = MSG_SIZE + 4;
    const FOOTER_SIZE: i32 = 14;
    const MAX_LINES: usize = 8;

    let lines: Vec<&str> = err.lines().collect();
    let shown = lines.len().min(MAX_LINES) as i32;
    let truncated = lines.len() > MAX_LINES;
    let footer = "fix & save to reload   \u{00b7}   F5 to reset";

    let content_h =
        TITLE_SIZE + 8 + shown * LINE_H + if truncated { LINE_H } else { 0 } + 10 + FOOTER_SIZE;
    let box_h = content_h + PADDING * 2;
    let box_y = screen_h - box_h;

    d.draw_rectangle(0, box_y, screen_w, box_h, Color::new(30, 10, 10, 235));
    d.draw_rectangle(0, box_y, screen_w, 2, Color::new(220, 60, 60, 255));

    let mut y = box_y + PADDING;
    d.draw_text(
        "Lua error",
        PADDING,
        y,
        TITLE_SIZE,
        Color::new(220, 60, 60, 255),
    );
    y += TITLE_SIZE + 8;

    for line in lines.iter().take(MAX_LINES) {
        d.draw_text(line, PADDING, y, MSG_SIZE, Color::WHITE);
        y += LINE_H;
    }
    if truncated {
        d.draw_text("\u{2026}", PADDING, y, MSG_SIZE, Color::WHITE);
        y += LINE_H;
    }

    y += 10;
    d.draw_text(
        footer,
        PADDING,
        y,
        FOOTER_SIZE,
        Color::new(180, 180, 180, 255),
    );
}

/// Install constant tables (`gfx`, `input`, `usagi`) on the Lua globals.
/// Per-frame closures (gfx.clear, input.pressed, ...) are installed inside
/// `lua.scope` blocks since they borrow frame-local Rust state.
fn setup_api(lua: &Lua) -> LuaResult<()> {
    let gfx = lua.create_table()?;
    gfx.set("COLOR_BLACK", 0)?;
    gfx.set("COLOR_DARK_BLUE", 1)?;
    gfx.set("COLOR_DARK_PURPLE", 2)?;
    gfx.set("COLOR_DARK_GREEN", 3)?;
    gfx.set("COLOR_BROWN", 4)?;
    gfx.set("COLOR_DARK_GRAY", 5)?;
    gfx.set("COLOR_LIGHT_GRAY", 6)?;
    gfx.set("COLOR_WHITE", 7)?;
    gfx.set("COLOR_RED", 8)?;
    gfx.set("COLOR_ORANGE", 9)?;
    gfx.set("COLOR_YELLOW", 10)?;
    gfx.set("COLOR_GREEN", 11)?;
    gfx.set("COLOR_BLUE", 12)?;
    gfx.set("COLOR_INDIGO", 13)?;
    gfx.set("COLOR_PINK", 14)?;
    gfx.set("COLOR_PEACH", 15)?;
    lua.globals().set("gfx", gfx)?;

    let input = lua.create_table()?;
    input.set("LEFT", KeyboardKey::KEY_LEFT as u32)?;
    input.set("RIGHT", KeyboardKey::KEY_RIGHT as u32)?;
    input.set("UP", KeyboardKey::KEY_UP as u32)?;
    input.set("DOWN", KeyboardKey::KEY_DOWN as u32)?;
    input.set("A", KeyboardKey::KEY_Z as u32)?;
    input.set("B", KeyboardKey::KEY_X as u32)?;
    lua.globals().set("input", input)?;

    // `sfx` table; `sfx.play` is installed per-frame inside lua.scope so the
    // closure can borrow the loaded sound map and audio device.
    let sfx = lua.create_table()?;
    lua.globals().set("sfx", sfx)?;

    // `gfx` and `input` are top-level globals (see above). The `usagi` table
    // is reserved for engine-level info — runtime constants, current frame
    // stats, etc. Not a namespace for the per-domain APIs.
    let usagi = lua.create_table()?;
    usagi.set("GAME_W", GAME_WIDTH)?;
    usagi.set("GAME_H", GAME_HEIGHT)?;
    lua.globals().set("usagi", usagi)?;

    Ok(())
}

fn main() -> LuaResult<()> {
    let script_arg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/hello_usagi.lua".to_string());
    let script_path = match resolve_script_path(&script_arg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[usagi] {}", e);
            std::process::exit(1);
        }
    };
    // Sprite sheet convention: `sprites.png` in the same directory as the
    // script. Optional — games without a PNG just have no sprites loaded.
    let sprites_path = std::path::Path::new(&script_path).with_file_name("sprites.png");
    // SFX convention: `sfx/` directory next to the script; each .wav file
    // becomes sfx.play("<stem>"). Missing dir is fine — just no sounds.
    let sfx_dir = std::path::Path::new(&script_path).with_file_name("sfx");

    let (mut rl, thread) = sola_raylib::init()
        .size((GAME_WIDTH * 2.) as i32, (GAME_HEIGHT * 2.) as i32)
        .highdpi()
        .resizable()
        .title("USAGI")
        .build();
    rl.set_target_fps(60);
    let mut rt: RenderTexture2D = rl
        .load_render_texture(&thread, GAME_WIDTH as u32, GAME_HEIGHT as u32)
        .unwrap();

    let lua = Lua::new();
    setup_api(&lua)?;
    // Latest Lua error, if any — rendered as an on-screen overlay and cleared
    // on successful reload or F5 reset.
    let mut last_error: Option<String> = None;

    // Initial load errors are non-fatal: boot with an empty session and let
    // the user fix the file + save to trigger live reload.
    record_err(
        &mut last_error,
        "initial load",
        load_script(&lua, &script_path),
    );

    if let Ok(init) = lua.globals().get::<LuaFunction>("_init") {
        record_err(&mut last_error, "_init", init.call::<()>(()));
    }
    let mut update: Option<LuaFunction> = lua.globals().get("_update").ok();
    let mut draw: Option<LuaFunction> = lua.globals().get("_draw").ok();
    let mut last_modified = std::fs::metadata(&script_path)
        .and_then(|m| m.modified())
        .ok();

    let mut sprites: Option<Texture2D> = load_sprites(&mut rl, &thread, &sprites_path);
    let mut sprites_mtime = std::fs::metadata(&sprites_path)
        .and_then(|m| m.modified())
        .ok();

    // Audio is optional — if the device can't be initialised, games still
    // run; sfx.play just no-ops.
    let audio = RaylibAudio::init_audio_device()
        .map_err(|e| eprintln!("[usagi] audio init failed: {}", e))
        .ok();
    let mut sounds = match &audio {
        Some(a) => load_sfx(a, &sfx_dir),
        None => std::collections::HashMap::new(),
    };
    let mut sfx_manifest = scan_sfx(&sfx_dir);

    while !rl.window_should_close() {
        // Live reload: on mtime change, re-exec the script on the same Lua
        // VM so _update / _draw pick up the new definitions next frame.
        //
        // State is intentionally preserved — we do NOT call _init() here.
        // The point of live reload during prototyping is to tweak logic
        // without losing the current play session (player position, enemy
        // state, etc.). F5 below is the explicit "reset" escape hatch.
        //
        // Caveat for script authors: top-level `local` bindings get fresh
        // nil values on re-exec, so callbacks that captured them will see
        // nil. State that needs to survive reload should live in globals
        // (or use `x = x or <init>` to preserve across re-exec).
        //
        // Reload errors are logged, not fatal — the previous callbacks keep
        // running so a half-saved or syntactically-broken file can't kill
        // the dev session.
        if let Ok(modified) = std::fs::metadata(&script_path).and_then(|m| m.modified())
            && Some(modified) != last_modified
        {
            last_modified = Some(modified);
            match load_script(&lua, &script_path) {
                Ok(()) => {
                    println!("[usagi] reloaded {}", script_path);
                    update = lua.globals().get("_update").ok();
                    draw = lua.globals().get("_draw").ok();
                    last_error = None;
                }
                Err(e) => {
                    let msg = format!("reload: {}", e);
                    eprintln!("[usagi] {}", msg);
                    last_error = Some(msg);
                }
            }
        }

        // Sprite sheet live reload: save in your image editor, see new pixels
        // next frame. Drop of the previous Texture2D frees its GPU memory.
        if let Ok(modified) = std::fs::metadata(&sprites_path).and_then(|m| m.modified())
            && Some(modified) != sprites_mtime
        {
            sprites_mtime = Some(modified);
            sprites = load_sprites(&mut rl, &thread, &sprites_path);
            println!("[usagi] reloaded {}", sprites_path.display());
        }

        // SFX live reload: scan the sfx dir each frame; if any file was
        // added/removed/edited (mtime change), reload the whole map. Cheap
        // since the scan is just stats; we only pay for new_sound when the
        // manifest actually differs.
        if let Some(ref a) = audio {
            let new_manifest = scan_sfx(&sfx_dir);
            if new_manifest != sfx_manifest {
                sfx_manifest = new_manifest;
                sounds = load_sfx(a, &sfx_dir);
                println!("[usagi] reloaded sfx ({} sound(s))", sounds.len());
            }
        }

        // Dev shortcut: F5 runs _init() to wipe game state. Paired with the
        // preserve-state live-reload above; this is the one way to actually
        // reset during a session without restarting the process.
        if rl.is_key_pressed(KeyboardKey::KEY_F5)
            && let Ok(init) = lua.globals().get::<LuaFunction>("_init")
        {
            match init.call::<()>(()) {
                Ok(()) => {
                    println!("[usagi] reset (F5)");
                    last_error = None;
                }
                Err(e) => {
                    let msg = format!("_init: {}", e);
                    eprintln!("[usagi] {}", msg);
                    last_error = Some(msg);
                }
            }
        }

        let dt = rl.get_frame_time();
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        let fps = rl.get_fps();

        // Update phase: input.pressed / input.down borrow rl (immutable — the
        // raylib calls are &self). Errors from user Lua are logged and swallowed
        // so a broken _update doesn't kill the session.
        if let Some(ref update_fn) = update {
            let rl_ref = &rl;
            let sounds_ref = &sounds;
            record_err(
                &mut last_error,
                "_update",
                lua.scope(|scope| {
                    let input_tbl: LuaTable = lua.globals().get("input")?;
                    let pressed = scope.create_function(|_, key: u32| {
                        Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_pressed(k)))
                    })?;
                    input_tbl.set("pressed", pressed)?;
                    let down = scope.create_function(|_, key: u32| {
                        Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_down(k)))
                    })?;
                    input_tbl.set("down", down)?;

                    let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                    let play = scope.create_function(|_, name: String| {
                        if let Some(sound) = sounds_ref.get(&name) {
                            sound.play();
                        }
                        Ok(())
                    })?;
                    sfx_tbl.set("play", play)?;

                    update_fn.call::<()>(dt)?;
                    Ok(())
                }),
            );
        }

        // Draw phase: gfx.* share d_rt via RefCell (multiple draw fns need mut
        // access). Errors from user Lua are logged and swallowed; the partial
        // RT contents still get blitted so the window stays alive.
        {
            let mut d_rt = rl.begin_texture_mode(&thread, &mut rt);
            if let Some(ref draw_fn) = draw {
                let d_rt_cell = std::cell::RefCell::new(&mut d_rt);
                let sprites_ref = sprites.as_ref();
                let sounds_ref = &sounds;
                record_err(
                    &mut last_error,
                    "_draw",
                    lua.scope(|scope| {
                        let gfx_tbl: LuaTable = lua.globals().get("gfx")?;
                        let clear = scope.create_function(|_, c: i32| {
                            d_rt_cell.borrow_mut().clear_background(palette(c));
                            Ok(())
                        })?;
                        let text =
                            scope.create_function(|_, (s, x, y, c): (String, f32, f32, i32)| {
                                d_rt_cell.borrow_mut().draw_text(
                                    &s,
                                    x.round() as i32,
                                    y.round() as i32,
                                    8,
                                    palette(c),
                                );
                                Ok(())
                            })?;
                        let rect = scope.create_function(
                            |_, (x, y, w, h, c): (f32, f32, f32, f32, i32)| {
                                d_rt_cell.borrow_mut().draw_rectangle(
                                    x.round() as i32,
                                    y.round() as i32,
                                    w.round() as i32,
                                    h.round() as i32,
                                    palette(c),
                                );
                                Ok(())
                            },
                        )?;
                        let spr = scope.create_function(|_, (idx, x, y): (i32, f32, f32)| {
                            // 1-based indexing to match Lua conventions
                            // (ipairs, t[1], string.sub). Sprite 1 is the
                            // top-left cell of the sheet.
                            if idx < 1 {
                                return Ok(());
                            }
                            let idx0 = idx - 1;
                            if let Some(tex) = sprites_ref {
                                const CELL: i32 = 16;
                                let cols = tex.width / CELL;
                                if cols <= 0 {
                                    return Ok(());
                                }
                                let col = idx0 % cols;
                                let row = idx0 / cols;
                                if row * CELL >= tex.height {
                                    return Ok(());
                                }
                                let source = Rectangle {
                                    x: (col * CELL) as f32,
                                    y: (row * CELL) as f32,
                                    width: CELL as f32,
                                    height: CELL as f32,
                                };
                                let pos = Vector2::new(x.round(), y.round());
                                d_rt_cell.borrow_mut().draw_texture_rec(
                                    tex,
                                    source,
                                    pos,
                                    Color::WHITE,
                                );
                            }
                            Ok(())
                        })?;
                        gfx_tbl.set("clear", clear)?;
                        gfx_tbl.set("text", text)?;
                        gfx_tbl.set("rect", rect)?;
                        gfx_tbl.set("spr", spr)?;

                        let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                        let play = scope.create_function(|_, name: String| {
                            if let Some(sound) = sounds_ref.get(&name) {
                                sound.play();
                            }
                            Ok(())
                        })?;
                        sfx_tbl.set("play", play)?;

                        draw_fn.call::<()>(dt)?;
                        Ok(())
                    }),
                );
            }
            d_rt.draw_text(&format!("FPS: {}", fps), 0, 0, 8, Color::GREEN);
        }

        // Blit render target to screen, then overlay any active Lua error.
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            draw_render_target(&mut d, &mut rt, screen_w, screen_h, true);
            if let Some(ref err) = last_error {
                draw_error_overlay(&mut d, err, screen_w, screen_h);
            }
        }
    }
    Ok(())
}
