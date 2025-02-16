use crate::item::Item;
use cursive::direction::Direction;
use cursive::event::{Callback, Event, EventResult, Key};
use cursive::theme::Theme;
use cursive::traits::*;
use cursive::view::CannotFocus;
use cursive::views::{Canvas, EditView, LinearLayout};
use cursive::{Cursive, Printer, Rect, Vec2};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::cmp::Reverse;
use std::sync::{Arc, Mutex};

struct UiState {
    query: String,
    idx: usize,

    rx: crossbeam_channel::Receiver<Arc<Item>>,
    all_items: Vec<Arc<Item>>,
    matches: Vec<Arc<Item>>,

    final_key: Option<Event>,
}

impl UiState {
    fn update(&mut self) {
        // drain rx
        loop {
            let item = match self.rx.try_recv() {
                Ok(item) => item,
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            };
            self.all_items.push(item);
        }

        // update matches
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let mut pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);

        let mut buf = Vec::new();
        let mut matches: Vec<_> = self
            .all_items
            .iter()
            .filter_map(|item| {
                pattern
                    .score(Utf32Str::new(item.text().as_ref(), &mut buf), &mut matcher)
                    .map(|score| (item, score))
            })
            .collect();
        matches.sort_by_key(|(_, score)| Reverse(*score));
        self.matches = matches
            .into_iter()
            .map(|(item, _)| Arc::clone(item))
            .collect();
    }
}

pub fn run_ui(rx: crossbeam_channel::Receiver<Arc<Item>>) -> (Option<Arc<Item>>, Option<Event>) {
    let mut siv = cursive::default();
    siv.set_theme(Theme::terminal_default());

    let ui_state = Arc::new(Mutex::new(UiState {
        query: String::new(),
        idx: 0,
        rx,
        all_items: Vec::with_capacity(1024),
        matches: Vec::new(),

        final_key: None,
    }));

    siv.add_global_callback(Event::Char('\\'), |s| s.quit());

    siv.add_fullscreen_layer(
        LinearLayout::vertical()
            .child(InputView::new(Arc::clone(&ui_state)).full_width())
            .child(
                LinearLayout::horizontal()
                    .child(
                        MatchesView {
                            ui_state: Arc::clone(&ui_state),
                        }
                        .full_width()
                        .full_height(),
                    )
                    .child(
                        Canvas::new(())
                            .with_draw(|_, printer| {
                                for i in 0..printer.output_size.y {
                                    printer.print((0, i), "\u{2502}"); // │ (long |)
                                }
                            })
                            .full_height()
                            .fixed_width(1),
                    )
                    .child(Canvas::new(()).full_width().full_height()),
            ),
    );

    siv.run();

    let ui_state = ui_state.lock().expect("panic");
    let focused = ui_state.idx;
    let item = ui_state.matches.get(focused).map(|item| Arc::clone(item));
    (item, ui_state.final_key.clone())
}

struct InputView {
    ui_state: Arc<Mutex<UiState>>,
    inner: EditView,
}

impl InputView {
    fn new(ui_state: Arc<Mutex<UiState>>) -> Self {
        let for_edit = Arc::clone(&ui_state);
        Self {
            ui_state,
            inner: EditView::default().on_edit_mut(move |_, s, _| {
                for_edit.lock().expect("panic").query = s.to_string();
            }),
        }
    }
}

impl View for InputView {
    fn draw(&self, printer: &Printer) {
        self.inner.draw(printer);
    }

    fn layout(&mut self, v: Vec2) {
        self.inner.layout(v);
    }

    fn on_event(&mut self, ev: Event) -> EventResult {
        let mut ui_state = self.ui_state.lock().expect("panic");
        ui_state.final_key = Some(ev.clone());
        ui_state.update();
        match ev {
            Event::Key(Key::Down) => {
                ui_state.idx += 1;
                return EventResult::Consumed(None);
            }
            Event::Key(Key::Up) => {
                ui_state.idx -= 1;
                return EventResult::Consumed(None);
            }
            Event::Key(Key::Enter) => {
                return EventResult::Consumed(Some(Callback::from_fn(|c: &mut Cursive| c.quit())));
            }
            _ => (),
        }
        drop(ui_state);
        self.inner.on_event(ev)
    }

    fn take_focus(&mut self, source: Direction) -> Result<EventResult, CannotFocus> {
        self.inner.take_focus(source)
    }

    fn important_area(&self, v: Vec2) -> Rect {
        self.inner.important_area(v)
    }
}

struct MatchesView {
    ui_state: Arc<Mutex<UiState>>,
}

impl View for MatchesView {
    fn layout(&mut self, _: Vec2) {}

    fn draw(&self, printer: &Printer) {
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let ui_state = self.ui_state.lock().expect("panic");

        printer.with_color(cursive::theme::ColorStyle::title_primary(), |printer| {
            printer.print((0, ui_state.idx), "> ");
        });

        for i in 0..printer.output_size.y.min(ui_state.matches.len()) {
            printer.print((2, i), &ui_state.matches[i].text());
        }
    }
}
