use std::{cell::RefCell, rc::Rc};

use cursive::{
    direction::{Direction, Orientation},
    event::{Event, EventResult, Key, MouseButton, MouseEvent},
    menu::MenuTree,
    theme::{BaseColor, Color, ColorStyle},
    utils::markup::StyledString,
    view::*,
    views::*,
    CbSink, Cursive, Printer, Vec2, XY,
};
use screeps_api::websocket::UserConsoleUpdate;
use futures::channel::mpsc::UnboundedSender;
use log::{debug, warn};
use screeps_api::MyInfo;

use crate::{
    net::Command,
    room::{ConnectionState, RoomId, VisualObject, VisualRoom},
};

mod console;
mod info;

mod ids {
    pub const CONN_STATE: &str = "conn-state";
    pub const SERVER_STATE: &str = "server-state";
    pub const USERNAME: &str = "username";
    pub const ROOM_ID: &str = "room-id";
    pub const LAST_UPDATE_TIME: &str = "last-update-game-time";
    pub const HOVER_INFO: &str = "hover-info";

    pub const SHARD_SELECT_LIST: &str = "shard-select-list";
}

#[derive(Clone, Debug, smart_default::SmartDefault)]
pub struct State {
    server: Option<String>,
    connection: Option<ConnectionState>,
    user_info: Option<MyInfo>,
    room: Option<VisualRoom>,
    send: Option<UnboundedSender<Command>>,
    shards: Option<Vec<String>>,
    /// Not the main storage for cursor (that's in RoomView), but a read-only version
    /// kept up to date for use by other views.
    #[default(_code = "XY::new(25, 25)")]
    cursor: XY<i32>,
    console: console::ConsoleState,
}

impl State {
    fn send_command(&mut self, command: Command) {
        match &mut self.send {
            Some(s) => match s.unbounded_send(command) {
                Ok(()) => (),
                Err(e) => warn!("couldn't send command to network thread: {}", e),
            },
            None => warn!("couldn't send command to network thread: no sender attached"),
        }
    }
}

pub struct CursiveStatePair<'a, 'b> {
    siv: &'a mut Cursive,
    state: &'b mut State,
}

impl<'a, 'b> CursiveStatePair<'a, 'b> {
    fn new(siv: &'a mut Cursive, state: &'b mut State) -> Self {
        CursiveStatePair { siv, state }
    }

    pub fn server(&mut self, server: String) {
        self.siv
            .find_id::<TextView>(ids::SERVER_STATE)
            .expect("expected to find SERVER_STATE view")
            .set_content(server.clone());
        self.state.server = Some(server);
    }

    pub fn user(&mut self, info: MyInfo) {
        self.siv
            .find_id::<TextView>(ids::USERNAME)
            .expect("expected to find USERNAME view")
            .set_content(info.username.clone());
        self.state.user_info = Some(info);
    }

    pub fn room(&mut self, room: VisualRoom) {
        if self.state.room.as_ref().map(|r| &r.room_id) != Some(&room.room_id) {
            self.siv
                .find_id::<TextView>(ids::ROOM_ID)
                .expect("expected to find ROOM_ID view")
                .set_content(room.room_id.to_string());
        }
        if let Some(updated) = room.last_update_time {
            self.siv
                .find_id::<TextView>(ids::LAST_UPDATE_TIME)
                .expect("expected to find LAST_UPDATE_TIME view")
                .set_content(format!("updated: {}", updated));
        }
        self.state.room = Some(room);
        self.update_hover_info();
    }

    pub fn conn_state(&mut self, state: ConnectionState) {
        self.state.connection = Some(state);
        let color = match state {
            ConnectionState::Authenticating => BaseColor::Yellow,
            ConnectionState::Connected => BaseColor::Green,
            ConnectionState::Disconnected => BaseColor::Red,
            ConnectionState::Error => BaseColor::Red,
        };

        self.siv
            .find_id::<TextView>(ids::CONN_STATE)
            .expect("expected to find CONN_STATE view")
            .set_content(StyledString::styled(state.to_string(), Color::Dark(color)));
    }

    pub fn shards(&mut self, shards: Vec<String>) {
        if self.state.shards.as_ref() != Some(&shards) {
            self.state.shards = Some(shards);
            self.shard_select_popup();
        }
    }

    fn shard_select_popup(&mut self) {
        if self
            .siv
            .find_id::<MenuPopup>(ids::SHARD_SELECT_LIST)
            .is_some()
        {
            return;
        }
        let shards = match &self.state.shards {
            None => {
                self.state.send_command(Command::FetchShardNames);
                return;
            }
            Some(v) => v,
        };
        let mut menu = MenuTree::new();
        for shard in shards.iter() {
            let cloned_shard = shard.clone();
            menu.add_leaf(&**shard, move |s| {
                debug!("changing shard to {}", cloned_shard);
                let cloned_shard = cloned_shard.clone();
                sync_update(s, |s| {
                    s.state.send_command(Command::ChangeShard(cloned_shard));
                });
            });
        }
        let popup = MenuPopup::new(Rc::new(menu));
        let layer = LinearLayout::new(Orientation::Vertical)
            .child(TextView::new("Choose a shard"))
            .child(popup.with_id(ids::SHARD_SELECT_LIST));
        self.siv.add_layer(layer);
        self.siv
            .focus(&Selector::Id(ids::SHARD_SELECT_LIST))
            .expect("just added shard list");
    }

    pub fn command_sender(&mut self, send: UnboundedSender<Command>) {
        self.state.send = Some(send);
    }

    /// Requires cursor to be between (0, 0) and (50, 50)
    pub fn cursor(&mut self, cursor: XY<i32>) {
        self.state.cursor = cursor;
        self.update_hover_info();
    }

    fn update_hover_info(&mut self) {
        if let Some(room) = &self.state.room {
            let things = room
                .objs
                .get((self.state.cursor.x as usize, self.state.cursor.y as usize))
                .expect("expected cursor passed in to be in valid range");

            let time = room.last_update_time.unwrap_or_default();

            let desc = info::info(things, &info::InfoInfo::new(time, &room.users));

            self.siv
                .find_id::<TextView>(ids::HOVER_INFO)
                .expect("expected to find HOVER_INFO view")
                .set_content(desc);
        }
    }

    pub fn console_update(&mut self, update: UserConsoleUpdate) {
        self.state.console.console_update(&mut self.siv, update);
    }
}

thread_local! {
    static STATE: RefCell<State> = Default::default();
}

/// Utility function for use updating the state with a CbSink.
pub fn async_update<F: FnOnce(&mut CursiveStatePair) + Send + 'static>(
    sink: &CbSink,
    func: F,
) -> Result<(), crate::net::Error> {
    sink.send(Box::new(|siv: &mut Cursive| {
        STATE.with(|state| {
            func(&mut CursiveStatePair::new(siv, &mut state.borrow_mut()));
        })
    }))
    .map_err(|e| format!("{}", e).into())
}

fn sync_update<F: FnOnce(&mut CursiveStatePair)>(siv: &mut Cursive, func: F) {
    STATE.with(|state| {
        func(&mut CursiveStatePair::new(siv, &mut state.borrow_mut()));
    })
}

pub fn setup(c: &mut Cursive) {
    let mut layout = LinearLayout::new(Orientation::Horizontal);
    layout.add_child(RoomView::new());

    let mut sidebar = LinearLayout::new(Orientation::Vertical);
    sidebar.add_child(TextView::new("").with_id(ids::SERVER_STATE));
    sidebar.add_child(TextView::new("").with_id(ids::CONN_STATE));
    sidebar.add_child(TextView::new("").with_id(ids::USERNAME));
    sidebar.add_child(TextView::new("").with_id(ids::ROOM_ID));
    sidebar.add_child(TextView::new("").with_id(ids::LAST_UPDATE_TIME));
    sidebar.add_child(
        TextView::new("")
            .with_id(ids::HOVER_INFO)
            .boxed(SizeConstraint::AtLeast(50), SizeConstraint::Free),
    );

    layout.add_child(sidebar);

    layout.add_child(STATE.with(|s| s.borrow().console.view()));

    // layout.add_child(
    //     DebugView::new()
    //         .boxed(SizeConstraint::AtMost(80), SizeConstraint::Free)
    //         .squishable(),
    // );

    c.add_layer(layout);
    c.add_global_callback('q', |c| c.quit());
    c.add_global_callback('s', |siv| sync_update(siv, |s| s.shard_select_popup()));
}

#[derive(Clone, Debug, smart_default::SmartDefault)]
struct RoomView {
    #[default(_code = "XY::new(26, 26)")]
    cursor: XY<i32>,
}

impl RoomView {
    pub fn new() -> Self {
        Self::default()
    }
}

impl View for RoomView {
    fn draw(&self, printer: &Printer) {
        STATE.with(|state| {
            let state = state.borrow();
            if let Some(room) = state.room.as_ref() {
                let rendered = room
                    .rendered_rows
                    .as_ref()
                    .expect("expected rows to be rendered");
                for (idx, row_text) in rendered.iter().enumerate() {
                    let pos = (1, idx + 1);
                    printer.print(pos, row_text);
                }
                let cursor_ui_pos = ((self.cursor.x + 1) as usize, (self.cursor.y + 1) as usize);
                let symbol_at_cursor = if self.cursor.x >= 0
                    && self.cursor.x < 50
                    && self.cursor.y >= 0
                    && self.cursor.y < 50
                {
                    VisualObject::multiple_to_symbol(
                        room.objs
                            .get((self.cursor.x as usize, self.cursor.y as usize))
                            .unwrap(),
                    )
                } else {
                    " "
                };
                printer.print_styled(
                    cursor_ui_pos,
                    From::from(&StyledString::styled(
                        symbol_at_cursor,
                        ColorStyle {
                            front: Color::Dark(BaseColor::Magenta).into(),
                            back: Color::Light(BaseColor::Cyan).into(),
                        },
                    )),
                );
            }
        });
    }

    fn on_event(&mut self, e: Event) -> EventResult {
        #[derive(Debug)]
        enum Move {
            Abs(i32, i32),
            Rel(i32, i32),
        }
        let change = match e {
            Event::Key(Key::Left) | Event::Char('h') => Move::Rel(-1, 0),
            Event::Key(Key::Right) | Event::Char('l') => Move::Rel(1, 0),
            Event::Key(Key::Up) | Event::Char('k') => Move::Rel(0, -1),
            Event::Key(Key::Down) | Event::Char('j') => Move::Rel(0, 1),
            Event::Mouse {
                offset,
                position,
                event: MouseEvent::Press(MouseButton::Left),
                ..
            } => Move::Abs(
                position.x as i32 - offset.x as i32 - 1,
                position.y as i32 - offset.y as i32 - 1,
            ),
            _ => return EventResult::Ignored,
        };

        debug!("canvas event: {:?}", change);

        match change {
            Move::Abs(x, y) => {
                self.cursor = XY::new(x, y);
            }
            Move::Rel(x, y) => {
                self.cursor.x += x;
                self.cursor.y += y;
            }
        }

        let rdx = self.cursor.x.div_euclid(50);
        // we treat negative values as "north", RoomName treats negative values as "south"
        let rdy = -self.cursor.y.div_euclid(50);
        self.cursor.x = self.cursor.x.rem_euclid(50);
        self.cursor.y = self.cursor.y.rem_euclid(50);

        STATE.with(|state| {
            let mut state = state.borrow_mut();

            if rdx != 0 || rdy != 0 {
                if let Some(visual_room) = &state.room {
                    let new_room_name = visual_room.room_id.room_name + (rdx, rdy);
                    let new_room = RoomId::new(visual_room.room_id.shard.clone(), new_room_name);
                    debug!("changing room from {} to {}", visual_room.room_id, new_room);
                    state.send_command(Command::ChangeRoom(new_room));
                }
            }
        });

        let cursor_to_send = self.cursor;
        EventResult::with_cb(move |siv| sync_update(siv, move |s| s.cursor(cursor_to_send)))
    }

    fn take_focus(&mut self, _dir: Direction) -> bool {
        true
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        Vec2::new(52, 52)
    }
}
