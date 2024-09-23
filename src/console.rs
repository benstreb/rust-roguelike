// Fake BTerm to act as a shim while switching between bracket_lib and ggez

use anyhow::Ok;
use ggez::{glam, graphics, input::keyboard, GameResult};

use crate::game_object::Color;

pub const PIXEL_SIZE: f32 = 14.;

pub type VirtualKeyCode = keyboard::KeyCode;

pub struct BTerm {
    canvas: Option<graphics::Canvas>,
}

impl BTerm {
    pub fn new(_ctx: &mut ggez::Context) -> BTerm {
        BTerm { canvas: None }
    }

    pub fn key(&self, ctx: &ggez::Context) -> Option<VirtualKeyCode> {
        ctx.keyboard
            .pressed_keys()
            .into_iter()
            .take(1)
            .find(|_| true)
            .copied()
    }

    pub fn quit(&self, ctx: &mut ggez::Context) {
        ctx.request_quit();
    }

    pub fn cls(&mut self, ctx: &mut ggez::Context) {
        self.canvas = Some(ggez::graphics::Canvas::from_frame(
            ctx,
            graphics::Color::from_rgb(0, 0, 0),
        ));
    }

    pub fn print(&mut self, x: i64, y: i64, s: &str) {
        let canvas = self.canvas.as_mut().expect("print called with no canvas");
        let fragment = graphics::TextFragment {
            text: s.to_owned(),
            color: Some(ggez::graphics::Color::WHITE),
            ..Default::default()
        };
        canvas.draw(
            graphics::Text::new(fragment).set_scale(PIXEL_SIZE),
            Self::to_pixel_coordinates(x, y),
        )
    }
    pub fn print_color(&mut self, x: i64, y: i64, fg_color: Color, bg_color: Color, s: &str) {
        let canvas = self.canvas.as_mut().expect("print called with no canvas");
        let top_left = Self::to_pixel_coordinates(x, y);

        let bg_box = graphics::Rect::new(
            top_left.x,
            top_left.y,
            PIXEL_SIZE * s.chars().count() as f32,
            PIXEL_SIZE,
        );
        canvas.draw(
            &graphics::Quad,
            graphics::DrawParam::new().dest_rect(bg_box).color(bg_color),
        );

        let fragment = graphics::TextFragment {
            text: s.to_owned(),
            color: Some(fg_color.into()),
            ..Default::default()
        };
        canvas.draw(
            graphics::Text::new(fragment).set_scale(PIXEL_SIZE),
            top_left,
        );
    }

    pub fn finish(&mut self, ctx: &mut ggez::Context) -> GameResult {
        if let Some(canvas) = self.canvas.take() {
            canvas.finish(ctx)?;
        }
        GameResult::Ok(())
    }

    fn to_pixel_coordinates(x: i64, y: i64) -> glam::Vec2 {
        glam::Vec2::new(x as f32 * 16., y as f32 * 16.)
    }
}
