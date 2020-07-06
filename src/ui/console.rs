use std::mem;

use cursive::{
    theme::{BaseColor, Color},
    utils::markup::StyledString,
    view::*,
    views::*,
    Cursive,
};
use debug_stub_derive::DebugStub;
use screeps_api::websocket::UserConsoleUpdate;
use smart_default::SmartDefault;

pub const CONSOLE_TEXT: &str = "console-text";
pub const MAX_LINES_TO_KEEP: u32 = 2000;

#[derive(Clone, DebugStub, SmartDefault)]
pub struct ConsoleState {
    /// split so that we can get rid of old lines without technically 'splitting' a string
    /// upper_lines will be filled as soon as lower_lines has 2000 characters.
    upper_lines: StyledString,
    lower_lines: StyledString,
    #[default(_code = "TextContent::new(StyledString::default())")]
    #[debug_stub = "content handle"]
    handle: TextContent,
    lines_in_lower: u32,
}

impl ConsoleState {
    pub fn view(&self) -> impl View + 'static {
        ScrollView::new(TextView::new_with_content(self.handle.clone()))
            .scroll_strategy(ScrollStrategy::StickToBottom)
            .show_scrollbars(false)
            .with_name(CONSOLE_TEXT)
            .resized(SizeConstraint::Fixed(80), SizeConstraint::Free)
    }

    pub fn console_update(&mut self, srv: &mut Cursive, update: UserConsoleUpdate) {
        let mut scroll = srv
            .find_name::<ScrollView<TextView>>(CONSOLE_TEXT)
            .expect("expected to find CONSOLE_TEXT view");
        if scroll.is_at_bottom() {
            scroll.set_scroll_strategy(ScrollStrategy::StickToBottom);
        }
        match update {
            UserConsoleUpdate::Messages {
                log_messages,
                result_messages,
                shard,
            } => {
                for msg in log_messages {
                    self.add_styled_message(Self::format_log_message(&shard, msg));
                }
                for msg in result_messages {
                    self.add_styled_message(Self::format_result_message(&shard, msg));
                }
            }
            UserConsoleUpdate::Error { message, shard } => {
                self.add_styled_message(Self::format_error_message(&shard, message));
            }
        }
    }

    fn add_styled_message(&mut self, line: StyledString) {
        if self.lines_in_lower >= MAX_LINES_TO_KEEP {
            self.upper_lines = mem::replace(&mut self.lower_lines, StyledString::new());
            self.handle.set_content(self.upper_lines.clone());
            self.lines_in_lower = 0;
        }
        self.lower_lines.append(line.clone());
        self.handle.append(line);
    }

    fn format_log_message(_shard: &Option<String>, msg: String) -> StyledString {
        // TODO: formatting
        StyledString::plain(format!("{}\n", msg))
    }

    fn format_result_message(_shard: &Option<String>, msg: String) -> StyledString {
        // TODO: formatting
        StyledString::plain(format!("{}\n", msg))
    }

    fn format_error_message(_shard: &Option<String>, msg: String) -> StyledString {
        // TODO: formatting
        StyledString::styled(format!("{}\n", msg), Color::Dark(BaseColor::Red))
    }
}
