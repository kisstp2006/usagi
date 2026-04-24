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
fn main() -> LuaResult<()> {
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

    while !rl.window_should_close() {
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        let fps = rl.get_fps();

        // Draw game to render target
        {
            let mut d_rt = rl.begin_texture_mode(&thread, &mut rt);
            d_rt.clear_background(Color::WHITE);

            let lua = Lua::new();
            lua.scope(|scope| {
                let draw_text =
                    scope.create_function_mut(|_, (text, x, y): (String, i32, i32)| {
                        d_rt.draw_text(&text, x, y, 8, Color::BLACK);
                        Ok(())
                    })?;
                lua.globals().set("draw_text", draw_text)?;
                lua.load("draw_text(\"Hello from Lua!\", 12, 12)").exec()?;
                Ok(())
            })?;

            d_rt.draw_text("Hello from Rust!", 12, 24, 8, Color::BLACK);
            d_rt.draw_text(&format!("FPS: {}", fps), 0, 0, 8, Color::GREEN);
        }

        // Draw render target to screen
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            draw_render_target(&mut d, &mut rt, screen_w, screen_h, true);
        }
    }
    Ok(())
}
