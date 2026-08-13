use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel};

use crate::entries::GameEntry;
use crate::GameViewData;

const COVER_WINDOW: usize = 2;
const COVER_MAX_DIM: u32 = 640;
/// How often the UI thread drains finished cover decodes.
const DRAIN_INTERVAL: Duration = Duration::from_millis(16);

type DecodeResult = (usize, Option<SharedPixelBuffer<Rgba8Pixel>>);

pub struct CoverState {
    games: Rc<Vec<GameEntry>>,
    model: Rc<VecModel<GameViewData>>,
    cache: RefCell<Vec<Option<Image>>>,
    loading: RefCell<Vec<bool>>,
    current: Cell<usize>,
    tx: RefCell<mpsc::Sender<DecodeResult>>,
    results: RefCell<mpsc::Receiver<DecodeResult>>,
    /// Kept alive as long as the state lives so decodes keep being applied.
    timer: RefCell<Option<Timer>>,
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

    /// Create the cover state for the given games and model, arm the UI-thread
    /// drain timer, and load the initial cover window.
    pub fn init(games: Rc<Vec<GameEntry>>, model: Rc<VecModel<GameViewData>>) -> Rc<Self> {
        let len = games.len();
        let (tx, rx) = mpsc::channel();
        let state = Rc::new(Self {
            games,
            model,
            cache: RefCell::new(vec![None; len]),
            loading: RefCell::new(vec![false; len]),
            current: Cell::new(0),
            tx: RefCell::new(tx),
            results: RefCell::new(rx),
            timer: RefCell::new(None),
        });

        // Worker threads cannot touch the Rc-based model (it is not Send), so
        // they send decoded buffers over the channel and a Timer owned here
        // applies them on the UI thread.
        let weak = Rc::downgrade(&state);
        let timer = Timer::default();
        {
            let weak = weak.clone();
            timer.start(TimerMode::Repeated, DRAIN_INTERVAL, move || {
                if let Some(state) = weak.upgrade() {
                    state.drain_results();
                }
            });
        }
        *state.timer.borrow_mut() = Some(timer);

        state.refresh();
        state
    }

    /// Move the cover window onto `index`, loading and unloading covers as
    /// needed. Covers are decoded on worker threads so the UI never blocks.
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
                    let tx = self.tx.borrow().clone();
                    std::thread::spawn(move || {
                        let buffer = decode_cover(&path, COVER_MAX_DIM);
                        let _ = tx.send((i, buffer));
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

    /// Apply every finished decode that is currently buffered.
    fn drain_results(&self) {
        let results: Vec<DecodeResult> = self.results.borrow_mut().try_iter().collect();
        for (index, buffer) in results {
            self.apply_cover(index, buffer);
        }
    }

    fn apply_cover(&self, index: usize, buffer: Option<SharedPixelBuffer<Rgba8Pixel>>) {
        let n = self.games.len();
        let current = self.current.get();
        let in_window =
            n <= 2 * COVER_WINDOW + 1 || circular_distance(index, current, n) <= COVER_WINDOW;

        if index >= self.loading.borrow().len() {
            return;
        }
        self.loading.borrow_mut()[index] = false;

        if !in_window {
            return;
        }

        let mut cache = self.cache.borrow_mut();
        if cache[index].is_some() {
            return;
        }

        let image = match buffer {
            Some(buffer) => Image::from_rgba8(buffer),
            None => Image::default(),
        };
        cache[index] = Some(image.clone());
        self.model.set_row_data(index, view_data(&self.games[index], image));
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

#[cfg(test)]
mod tests {
    use super::circular_distance;

    #[test]
    fn zero_items_is_max() {
        assert_eq!(circular_distance(0, 0, 0), usize::MAX);
    }

    #[test]
    fn same_index_is_zero() {
        assert_eq!(circular_distance(3, 3, 7), 0);
    }

    #[test]
    fn wraps_around_both_directions() {
        assert_eq!(circular_distance(1, 0, 5), 1);
        assert_eq!(circular_distance(4, 0, 5), 1);
        assert_eq!(circular_distance(3, 0, 5), 2);
        assert_eq!(circular_distance(0, 4, 5), 1);
    }
}