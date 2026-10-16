//! Holding a finger on a song for half a second opens its menu, as a right click does on a
//! computer. The finger has to stay put: moving it scrolls or swipes instead. The release
//! that ends the hold does not also press the row.

use std::cell::Cell;
use std::time::Duration;

use freya::prelude::*;

use crate::library::Song;
use crate::runtime;

/// How long a finger must stay down.
const HOLD: Duration = Duration::from_millis(500);
/// How far it may drift and still be a hold, in logical pixels.
const SLOP: f64 = 10.;

thread_local! {
    /// The hold under way: its number and where the finger went down.
    static HELD: Cell<Option<(u64, (f64, f64))>> = const { Cell::new(None) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
    /// Whether the last hold opened the menu, so its release is not a tap.
    static TAKEN: Cell<bool> = const { Cell::new(false) };
}

/// `element` opening `song`'s menu when a finger holds it.
pub fn hold(element: Rect, song: Song) -> Rect {
    element
        .on_pointer_down(move |event: Event<PointerEventData>| down(&event, song.clone()))
        .on_pointer_move(|event: Event<PointerEventData>| moved(&event))
        // Lifted anywhere before the time is up, it was no hold.
        .on_global_pointer_up(|_: Event<PointerEventData>| HELD.set(None))
}

/// Whether the press that just ended was a hold that opened the menu. Asked once per press.
pub fn taken() -> bool {
    TAKEN.replace(false)
}

fn down(event: &PointerEventData, song: Song) {
    TAKEN.set(false);
    // A mouse has its right click.
    let PointerEventData::Touch(touch) = event else {
        return;
    };
    let at = touch.global_location;
    let id = COUNT.get() + 1;
    COUNT.set(id);
    HELD.set(Some((id, (at.x, at.y))));
    spawn_in_window(async move {
        let _ = runtime::spawn(async { tokio::time::sleep(HOLD).await }).await;
        if HELD.get().is_some_and(|(held, _)| held == id) {
            HELD.set(None);
            TAKEN.set(true);
            crate::menu::open(song, (at.x as f32, at.y as f32));
        }
    });
}

fn moved(event: &PointerEventData) {
    let Some((_, (x, y))) = HELD.get() else {
        return;
    };
    let at = event.global_location();
    if (at.x - x).hypot(at.y - y) > SLOP {
        HELD.set(None);
    }
}
