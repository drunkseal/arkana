use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::entries::GameEntry;
use crate::GameViewData;

const COVER_WINDOW: usize = 2;
const COVER_MAX_DIM: u32 = 640;

pub struct CoverState {
    games: Rc<Vec<GameEntry>>,
    model: Rc<VecModel<GameViewData>>,
    cache: RefCell<Vec<Option<Image>>>,
    loading: RefCell<Vec<bool>>,
    current: Cell<usize>,
}

thread_local! {
    static COVER_STATE: RefCell<Option<Rc<CoverState>>> = const { RefCell::new(None) };
}

impl CoverState {
    /// Build the game list model with placeholder covers.
    pub fn build_model(games: &[GameEntry]) -> Rc<VecModel<GameViewData>> {
        Rc::new(VecModel::from(
            games
                .iter()
                .map(|game| view_data(game, Image::default()))
                .collect::<Vec<_>>(),
        ))
    }

    /// Create the cover state for the given games and model, register it for
    /// worker-thread callbacks, and load the initial cover window.
    pub fn init(games: Rc<Vec<GameEntry>>, model: Rc<VecModel<GameViewData>>) -> Rc<Self> {
        let len = games.len();
        let state = Rc::new(Self {
            games,
            model,
            cache: RefCell::new(vec![None; len]),
            loading: RefCell::new(vec![false; len]),
            current: Cell::new(0),
        });
        COVER_STATE.with(|s| *s.borrow_mut() = Some(state.clone()));
        state.refresh();
        state
    }

    /// Move the cover window onto `index`, loading and unloading covers as
    /// needed. Covers are decoded on a worker thread so the UI never blocks.
    pub fn set_current(&self, index: usize) {
        self.current.set(index);
        self.refresh();
    }

    fn refresh(&self) {
        let n = self.games.len();
        let mut cache = self.cache.borrow_mut();
        let mut loading = self.loading.borrow_mut();
        let current = self.current.get();

        for (i, game) in self.games.iter().enumerate() {
            let in_window =
                n <= 2 * COVER_WINDOW + 1 || circular_distance(i, current, n) <= COVER_WINDOW;

            if in_window && cache[i].is_none() && !loading[i] {
                if let Some(path) = game.cover.as_deref() {
                    loading[i] = true;
                    let path = path.to_path_buf();
                    std::thread::spawn(move || {
                        let buffer = decode_cover(&path, COVER_MAX_DIM);
                        let _ = slint::invoke_from_event_loop(move || apply_cover(i, buffer));
                    });
                } else {
                    cache[i] = Some(Image::default());
                }
            } else if !in_window && cache[i].is_some() {
                cache[i] = None;
                loading[i] = false;
                self.model.set_row_data(i, view_data(game, Image::default()));
            }
        }
    }
}

/// Decodes one cover file on a background thread.
fn decode_cover(path: &Path, max_dim: u32) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let img = image::open(path).ok()?;
    let rgba = img.thumbnail(max_dim, max_dim).to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    buffer.make_mut_bytes().copy_from_slice(rgba.as_raw());
    Some(buffer)
}

/// Applies a decoded cover back onto the model, on the UI thread.
fn apply_cover(index: usize, buffer: Option<SharedPixelBuffer<Rgba8Pixel>>) {
    COVER_STATE.with(|slot| {
        let Some(state) = slot.borrow().clone() else { return };
        let n = state.games.len();
        let current = state.current.get();
        let in_window =
            n <= 2 * COVER_WINDOW + 1 || circular_distance(index, current, n) <= COVER_WINDOW;

        if index >= state.loading.borrow().len() {
            return;
        }
        state.loading.borrow_mut()[index] = false;

        if !in_window {
            return;
        }

        let mut cache = state.cache.borrow_mut();
        if cache[index].is_some() {
            return;
        }

        let image = match buffer {
            Some(buffer) => Image::from_rgba8(buffer),
            None => Image::default(),
        };
        cache[index] = Some(image.clone());
        state.model.set_row_data(index, view_data(&state.games[index], image));
    });
}

fn view_data(game: &GameEntry, cover_art: Image) -> GameViewData {
    GameViewData {
        title: game.name.clone().into(),
        game_id: game.id as i32,
        cover_art,
    }
}

fn circular_distance(a: usize, b: usize, n: usize) -> usize {
    if n == 0 {
        return usize::MAX;
    }
    let forward = (a + n - b) % n;
    forward.min(n - forward)
}