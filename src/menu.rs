use canvas::Canvas;
use TouchReceiver;
use Visible;
use WorldPoint;

pub trait Action {
    fn start(self: Box<Self>, WorldPoint) -> Option<Box<TouchReceiver>>;
}

pub struct Entry {
    pub name: String,
    pub color: Option<String>,
    pub shortcuts: Vec<String>,
    pub action: Box<Action>,
}

pub struct Menu {
    pub entries: Vec<Entry>,
    pub color: String,
}

impl Menu {
    pub fn activate_shortcut(self, used_shortcut: String, point: WorldPoint) -> Option<Box<TouchReceiver>> {
        self.entries.into_iter()
            .find(
                |entry| entry.shortcuts.iter().any(
                    |entry_shortcut| &used_shortcut == entry_shortcut))
            .and_then(move |entry| entry.action.start(point))
    }
}

pub struct VisibleMenu {
    menu: Menu,
    page: usize,
    last_touch: WorldPoint,
}

impl Visible for VisibleMenu {
    fn draw(&self, c: &mut Canvas) {
    }

    fn start_touch(&self, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
        return None
    }
}

impl TouchReceiver for VisibleMenu {
    fn continue_touch(self: Box<Self>, p: WorldPoint) -> Option<Box<TouchReceiver>> {
        Some(self)
    }
    fn end_touch(self: Box<Self>) {}
}
