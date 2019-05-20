use std::cell::RefCell;

use cursive::{
    direction::Orientation,
    theme::{BaseColor, Color},
    utils::markup::StyledString,
    view::*,
    views::*,
    CbSink, Cursive, Printer,
};
use screeps_api::{websocket::types::room::objects::KnownRoomObject, MyInfo};

use crate::room::{ConnectionState, InterestingTerrainType, VisualObject, VisualRoom};

mod ids {
    pub const CONN_STATE: &str = "conn-state";
    pub const SERVER_STATE: &str = "server-state";
    pub const USERNAME: &str = "username";
    pub const ROOM_ID: &str = "room-id";
    pub const LAST_UPDATE_TIME: &str = "last-update-game-time";
}

#[derive(Clone, Debug, Default)]
pub struct State {
    server: Option<String>,
    connection: Option<ConnectionState>,
    user_info: Option<MyInfo>,
    room: Option<VisualRoom>,
}

pub struct CursiveStatePair<'a, 'b> {
    siv: &'a mut Cursive,
    state: &'b mut State,
}

impl CursiveStatePair<'_, '_> {
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
            func(&mut CursiveStatePair {
                siv,
                state: &mut state.borrow_mut(),
            });
        });
    }))
    .map_err(From::from)
}

pub fn setup(c: &mut Cursive) {
    let mut layout = LinearLayout::new(Orientation::Horizontal);
    layout.add_child(BoxView::new(
        SizeConstraint::Fixed(52),
        SizeConstraint::Fixed(52),
        Canvas::new(()).with_draw(draw_room),
    ));

    let mut sidebar = LinearLayout::new(Orientation::Vertical);
    sidebar.add_child(TextView::new("").with_id(ids::SERVER_STATE));
    sidebar.add_child(TextView::new("").with_id(ids::CONN_STATE));
    sidebar.add_child(TextView::new("").with_id(ids::USERNAME));
    sidebar.add_child(TextView::new("").with_id(ids::ROOM_ID));
    sidebar.add_child(TextView::new("").with_id(ids::LAST_UPDATE_TIME));

    layout.add_child(sidebar);

    c.add_layer(layout);
}

fn to_symbol(thing: &VisualObject) -> &'static str {
    match thing {
        VisualObject::InterestingTerrain {
            ty: InterestingTerrainType::Swamp,
            ..
        } => "~",
        VisualObject::InterestingTerrain {
            ty: InterestingTerrainType::Wall,
            ..
        } => "█",
        VisualObject::Flag(_) => "F",
        VisualObject::RoomObject(obj) => match obj {
            KnownRoomObject::Container(..) => "B",
            KnownRoomObject::Controller(..) => "C",
            KnownRoomObject::Creep(..) => "⚬",
            KnownRoomObject::Extension(..) => "E",
            KnownRoomObject::Extractor(..) => "X",
            KnownRoomObject::KeeperLair(..) => "K",
            KnownRoomObject::Lab(..) => "L",
            KnownRoomObject::Link(..) => "I",
            KnownRoomObject::Mineral(..) => "M",
            KnownRoomObject::Nuker(..) => "N",
            KnownRoomObject::Observer(..) => "O",
            KnownRoomObject::Portal(..) => "P",
            KnownRoomObject::PowerBank(..) => "B",
            KnownRoomObject::PowerSpawn(..) => "R",
            KnownRoomObject::Rampart(..) => "[",
            KnownRoomObject::Resource(..) => ".",
            KnownRoomObject::Road(..) => "-",
            KnownRoomObject::Source(..) => "S",
            KnownRoomObject::Spawn(..) => "P",
            KnownRoomObject::Storage(..) => "O",
            KnownRoomObject::Terminal(..) => "T",
            KnownRoomObject::Tower(..) => "♜",
            KnownRoomObject::Tombstone(..) => "⚰️",
            KnownRoomObject::Wall(..) => "W",
        },
    }
}

fn draw_room(_: &(), printer: &Printer) {
    STATE.with(|state| {
        let state = state.borrow();
        if let Some(room) = state.room.as_ref() {
            for (pos, objs) in room.objs.indexed_iter() {
                let (x, y) = (pos.0 + 1, pos.1 + 1);
                if let Some(obj) = objs.last() {
                    printer.print((x, y), to_symbol(obj));
                } else {
                    printer.print((x, y), " ");
                }
            }
        }
    });
}
