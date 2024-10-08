use std::collections::HashSet;

// BTerm shim layer
use ggez::{glam, graphics, input::keyboard, GameResult};

use crate::game_object::{self, Color};
use crate::meta;

pub const PIXEL_SIZE: f32 = 16.;
pub const PIXEL_WIDTH: f32 = 8.475;
pub const PIXEL_HEIGHT: f32 = 16.;
pub type VirtualKeyCode = keyboard::KeyCode;
pub type ClickType = ggez::event::MouseButton;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConsolePoint {
    pub x: i64,
    pub y: i64,
}

impl ConsolePoint {
    pub fn down(&self, n: i64) -> ConsolePoint {
        ConsolePoint {
            x: self.x,
            y: self.y + n,
        }
    }
}
impl From<game_object::WorldPoint> for ConsolePoint {
    fn from(pos: game_object::WorldPoint) -> Self {
        ConsolePoint {
            x: pos.x + meta::WORLD_TOP_LEFT.x,
            y: pos.y + meta::WORLD_TOP_LEFT.y,
        }
    }
}

impl From<ggez::mint::Point2<f32>> for ConsolePoint {
    fn from(pos: ggez::mint::Point2<f32>) -> Self {
        return ConsolePoint {
            x: (pos.x / PIXEL_WIDTH) as i64,
            y: (pos.y / PIXEL_HEIGHT) as i64,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClickEvent {
    pub click_type: ClickType,
    pub pos: ConsolePoint,
}

pub struct Console {
    canvas: Option<graphics::Canvas>,
    handled_keys: HashSet<VirtualKeyCode>,
    handled_clicks: HashSet<ClickType>,
}

impl Console {
    pub fn new(_ctx: &mut ggez::Context) -> Console {
        Console {
            canvas: None,
            handled_keys: HashSet::new(),
            handled_clicks: HashSet::new(),
        }
    }

    pub fn key_presses(&mut self, ctx: &ggez::Context) -> HashSet<VirtualKeyCode> {
        let keys = ctx.keyboard.pressed_keys();
        let new_keys = keys
            .difference(&self.handled_keys)
            .copied()
            .collect::<HashSet<VirtualKeyCode>>();
        self.handled_keys = keys.clone();
        new_keys
    }

    pub fn clicks(&mut self, ctx: &ggez::Context) -> HashSet<ClickEvent> {
        let mut clicks = HashSet::new();
        if ctx.mouse.button_pressed(ClickType::Left) {
            clicks.insert(ClickType::Left);
        }
        if ctx.mouse.button_pressed(ClickType::Right) {
            clicks.insert(ClickType::Right);
        }

        let new_clicks = clicks
            .difference(&self.handled_clicks)
            .copied()
            .collect::<HashSet<ClickType>>();
        self.handled_clicks = clicks.clone();

        new_clicks
            .into_iter()
            .map(|click_type| ClickEvent {
                pos: ctx.mouse.position().into(),
                click_type,
            })
            .collect::<HashSet<ClickEvent>>()
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

    pub fn print(&mut self, pos: ConsolePoint, s: &str) {
        let canvas = self.canvas.as_mut().expect("print called with no canvas");
        let fragment = graphics::TextFragment {
            text: s.to_owned(),
            color: Some(ggez::graphics::Color::WHITE),
            ..Default::default()
        };
        canvas.draw(
            graphics::Text::new(fragment).set_scale(PIXEL_SIZE),
            Self::to_pixel_coordinates(pos.x, pos.y),
        )
    }
    pub fn print_color(&mut self, pos: ConsolePoint, fg_color: Color, bg_color: Color, s: &str) {
        let canvas = self.canvas.as_mut().expect("print called with no canvas");
        let top_left = Self::to_pixel_coordinates(pos.x, pos.y);

        let bg_box = graphics::Rect::new(
            top_left.x,
            top_left.y,
            PIXEL_WIDTH * s.chars().count() as f32,
            PIXEL_HEIGHT,
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
        glam::Vec2::new(x as f32 * PIXEL_WIDTH, y as f32 * PIXEL_HEIGHT)
    }
}
