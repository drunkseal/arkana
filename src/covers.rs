use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

use slint::Model;

use crate::entries::GameEntry;
use crate::GameViewData;

pub const COVER_WINDOW: usize = 2;
pub const COVER_MAX_DIM: u32 = 640;

pub fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> slint::Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hp = hue / 60.0;
    let x = chroma * (1.0 - ((hp % 2.0) - 1.0).abs());

    let sector = (hp as u32) % 6;
    let (r1, g1, b1) = match sector {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };

    let m = lightness - chroma / 2.0;
    let r = ((r1 + m) * 255.0).round() as u8;
    let g = ((g1 + m) * 255.0).round() as u8;
    let b = ((b1 + m) * 255.0).round() as u8;

    slint::Color::from_rgb_u8(r, g, b)
}

pub fn placeholder_colors(name: &str) -> (slint::Color, slint::Color) {
    const SATURATION: f32 = 0.55;
    const TOP_LIGHTNESS: f32 = 0.30;
    const BOTTOM_LIGHTNESS: f32 = 0.08;

    let mut hash: u32 = 0;
    for b in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    let hue = (hash % 360) as f32;

    (
        hsl_to_rgb(hue, SATURATION, TOP_LIGHTNESS),
        hsl_to_rgb(hue, SATURATION, BOTTOM_LIGHTNESS),
    )
}

pub fn circular_distance(a: usize, b: usize, n: usize) -> usize {
    if n == 0 {
        return usize::MAX;
    }
    let forward = (a + n - b) % n;
    forward.min(n - forward)
}

pub fn make_view_data(game: &GameEntry, cover_art: slint::Image) -> GameViewData {
    let (c1, c2) = placeholder_colors(&game.name);
    let initial = game
        .name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();

    let has_cover = cover_art.size().width > 0 && cover_art.size().height > 0;

    GameViewData {
        game_id: game.id as i32,
        title: game.name.as_str().into(),
        initial: initial.as_str().into(),
        cover_art,
        has_cover,
        c1,
        c2,
    }
}

pub struct DecodedCover {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct CoverState {
    games: Vec<GameEntry>,
    model: Rc<slint::VecModel<GameViewData>>,
    cache: Vec<Option<slint::Image>>,
    loading: Vec<bool>,
    current: usize,
    result_sender: Sender<(usize, Option<DecodedCover>)>,
    result_receiver: Receiver<(usize, Option<DecodedCover>)>,
    _drain_timer: slint::Timer,
}

impl CoverState {
    pub fn build_model(games: &[GameEntry]) -> Rc<slint::VecModel<GameViewData>> {
        let items: Vec<GameViewData> = games
            .iter()
            .map(|g| make_view_data(g, slint::Image::default()))
            .collect();
        Rc::new(slint::VecModel::from(items))
    }

    pub fn init(
        games: Vec<GameEntry>,
        model: Rc<slint::VecModel<GameViewData>>,
    ) -> Rc<RefCell<Self>> {
        let count = games.len();
        let (tx, rx) = mpsc::channel();
        let drain_timer = slint::Timer::default();

        let state = Rc::new(RefCell::new(Self {
            games,
            model,
            cache: vec![None; count],
            loading: vec![false; count],
            current: 0,
            result_sender: tx,
            result_receiver: rx,
            _drain_timer: drain_timer,
        }));

        let weak = Rc::downgrade(&state);
        state.borrow()._drain_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            move || {
                if let Some(s) = weak.upgrade() {
                    s.borrow_mut().drain_results();
                }
            },
        );

        state.borrow_mut().refresh();
        state
    }

    pub fn set_current(&mut self, index: usize) {
        self.current = index;
        self.refresh();
    }

    pub fn refresh(&mut self) {
        let n = self.games.len();
        for i in 0..n {
            let in_window = (n <= 2 * COVER_WINDOW + 1)
                || (circular_distance(i, self.current, n) <= COVER_WINDOW);

            if in_window && self.cache[i].is_none() && !self.loading[i] {
                if let Some(ref path_str) = self.games[i].cover {
                    self.loading[i] = true;
                    let path = PathBuf::from(path_str);
                    let tx = self.result_sender.clone();
                    std::thread::spawn(move || {
                        let decoded = (|| -> Option<DecodedCover> {
                            let dynamic_img = image::ImageReader::open(&path).ok()?.decode().ok()?;
                            let (w, h) = (dynamic_img.width(), dynamic_img.height());
                            let (w, h, rgba) = if w > COVER_MAX_DIM || h > COVER_MAX_DIM {
                                let resized = dynamic_img.thumbnail(COVER_MAX_DIM, COVER_MAX_DIM);
                                (resized.width(), resized.height(), resized.to_rgba8().into_raw())
                            } else {
                                (w, h, dynamic_img.to_rgba8().into_raw())
                            };
                            Some(DecodedCover { width: w, height: h, rgba })
                        })();
                        let _ = tx.send((i, decoded));
                    });
                } else {
                    self.cache[i] = Some(slint::Image::default());
                }
            } else if !in_window && self.cache[i].is_some() {
                self.cache[i] = None;
                self.loading[i] = false;
                self.model
                    .set_row_data(i, make_view_data(&self.games[i], slint::Image::default()));
            }
        }
    }

    fn drain_results(&mut self) {
        while let Ok((index, decoded)) = self.result_receiver.try_recv() {
            self.apply_cover(index, decoded);
        }
    }

    fn apply_cover(&mut self, index: usize, decoded: Option<DecodedCover>) {
        let n = self.games.len();
        if index >= self.loading.len() {
            return;
        }

        self.loading[index] = false;

        let in_window = (n <= 2 * COVER_WINDOW + 1)
            || (circular_distance(index, self.current, n) <= COVER_WINDOW);
        if !in_window {
            return;
        }
        if self.cache[index].is_some() {
            return;
        }

        let img = if let Some(d) = decoded {
            let mut pixel_buffer =
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(d.width, d.height);
            pixel_buffer.make_mut_bytes().copy_from_slice(&d.rgba);
            slint::Image::from_rgba8(pixel_buffer)
        } else {
            slint::Image::default()
        };

        self.cache[index] = Some(img.clone());
        self.model
            .set_row_data(index, make_view_data(&self.games[index], img));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_distance() {
        assert_eq!(circular_distance(0, 0, 5), 0);
        assert_eq!(circular_distance(0, 1, 5), 1);
        assert_eq!(circular_distance(0, 4, 5), 1);
        assert_eq!(circular_distance(0, 2, 5), 2);
        assert_eq!(circular_distance(0, 3, 5), 2);
    }

    #[test]
    fn test_placeholder_colors() {
        let (c1, c2) = placeholder_colors("Sonic");
        assert_ne!(c1, c2);
    }
}
