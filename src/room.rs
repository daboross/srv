//! Room state
use std::{
    cmp::{Ordering, PartialOrd},
    collections::{hash_map::Entry, HashMap},
    fmt,
    sync::Arc,
};

use err_ctx::ResultExt;
use log::debug;
use ndarray::{Array, Ix2};
use screeps_api::{
    websocket::{flags::Flag, objects::KnownRoomObject, RoomUpdate, RoomUserInfo},
    RoomName, RoomTerrain, TerrainType,
};

use crate::net::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomId {
    pub shard: Option<String>,
    pub room_name: RoomName,
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.shard {
            Some(s) => write!(f, "{}:{}", s, self.room_name),
            None => write!(f, "{}", self.room_name),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, smart_default::SmartDefault, derive_more::Display)]
pub enum ConnectionState {
    #[default]
    #[display(fmt = "disconnected")]
    Disconnected,
    #[display(fmt = "authenticating...")]
    Authenticating,
    #[display(fmt = "connected")]
    Connected,
    #[display(fmt = "network error occurred, see log")]
    Error,
}

impl RoomId {
    pub fn new(shard: Option<String>, room_name: RoomName) -> Self {
        RoomId { shard, room_name }
    }
}

#[derive(Clone, Debug)]
pub struct Room {
    last_update_time: Option<u32>,
    room: RoomId,
    terrain: RoomTerrain,
    objects: HashMap<String, Arc<KnownRoomObject>>,
    flags: Vec<Flag>,
    users: HashMap<String, Arc<RoomUserInfo>>,
}

impl Room {
    pub fn new(room: RoomId, terrain: RoomTerrain) -> Self {
        assert_eq!(room.room_name, terrain.room_name);
        Room {
            last_update_time: None,
            room,
            terrain,
            objects: HashMap::new(),
            flags: Vec::new(),
            users: HashMap::new(),
        }
    }

    pub fn update(&mut self, update: RoomUpdate) -> Result<(), Error> {
        debug!("updating metadata");
        if let Some(time) = update.game_time {
            self.last_update_time = Some(time);
        }
        debug!("updating objects");
        for (id, data) in update.objects.into_iter() {
            if data.is_null() {
                self.objects.remove(&id);
            } else {
                match self.objects.entry(id.clone()) {
                    Entry::Occupied(entry) => {
                        Arc::make_mut(entry.into_mut())
                            .update(data.clone())
                            .with_ctx(|_| {
                                format!(
                                    "updating {} with data {}",
                                    id,
                                    serde_json::to_string(&data).unwrap()
                                )
                            })?;
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Arc::new(serde_json::from_value(data.clone()).with_ctx(
                            |_| {
                                format!(
                                    "creating {} with data {}",
                                    id,
                                    serde_json::to_string(&data).unwrap()
                                )
                            },
                        )?));
                    }
                }
            }
        }
        debug!("updating flags");
        self.flags = update.flags;

        debug!("updating users");
        for (user_id, data) in update.users.into_iter().flat_map(|x| x) {
            if data.is_null() {
                self.users.remove(&user_id);
            } else {
                match self.users.entry(user_id.clone()) {
                    Entry::Occupied(entry) => {
                        Arc::make_mut(entry.into_mut()).update(
                            serde_json::from_value(data.clone()).with_ctx(|_| {
                                format!(
                                    "updating user {} with data {}",
                                    user_id,
                                    serde_json::to_string(&data).unwrap(),
                                )
                            })?,
                        );
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Arc::new(serde_json::from_value(data.clone()).with_ctx(
                            |_| {
                                format!(
                                    "creating user {} with data {}",
                                    user_id,
                                    serde_json::to_string(&data).unwrap(),
                                )
                            },
                        )?));
                    }
                }
            }
        }

        debug!("update complete");

        Ok(())
    }

    pub fn visualize(&self) -> VisualRoom {
        let mut room =
            VisualRoom::new(self.last_update_time, self.room.clone(), self.users.clone());

        for (row_idx, row) in self.terrain.terrain.iter().enumerate() {
            for (col_idx, item) in row.iter().enumerate() {
                if let Some(itt) = InterestingTerrainType::from_terrain(*item) {
                    room.push_top(VisualObject::InterestingTerrain {
                        x: col_idx as u32,
                        y: row_idx as u32,
                        ty: itt,
                    });
                }
            }
        }

        for flag in &self.flags {
            room.push_top(VisualObject::Flag(flag.clone()));
        }

        for obj in self.objects.values() {
            room.push_top(VisualObject::RoomObject(obj.clone()));
        }

        for list in room.objs.iter_mut() {
            list.sort_unstable();
        }

        room.render_rows();

        room
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum RoomObjectType {
    Road,
    Container,
    Tombstone,
    Resource,
    Rampart,
    ConstructionSite,
    Wall,
    Source,
    Mineral,
    KeeperLair,
    Controller,
    Extractor,
    Extension,
    Spawn,
    Portal,
    Link,
    Storage,
    Tower,
    Observer,
    PowerBank,
    PowerSpawn,
    Lab,
    Terminal,
    Nuker,
    Creep,
}

impl RoomObjectType {
    pub fn of(obj: &KnownRoomObject) -> Self {
        macro_rules! transformit {
            ( $($id:ident),* $(,)? ) => {
                match obj {
                    $(
                        KnownRoomObject::$id(_) => RoomObjectType::$id,
                    )*
                }
            };
        }
        transformit!(
            Road,
            Container,
            Tombstone,
            ConstructionSite,
            Resource,
            Rampart,
            Wall,
            Source,
            Mineral,
            KeeperLair,
            Controller,
            Extractor,
            Extension,
            Spawn,
            Portal,
            Link,
            Storage,
            Tower,
            Observer,
            PowerBank,
            PowerSpawn,
            Lab,
            Terminal,
            Nuker,
            Creep,
        )
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum InterestingTerrainType {
    Swamp,
    Wall,
}

impl InterestingTerrainType {
    pub fn from_terrain(terrain: TerrainType) -> Option<Self> {
        match terrain {
            TerrainType::Plains => None,
            TerrainType::Swamp => Some(InterestingTerrainType::Swamp),
            TerrainType::Wall | TerrainType::SwampyWall => Some(InterestingTerrainType::Wall),
        }
    }
}

impl fmt::Display for InterestingTerrainType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InterestingTerrainType::Swamp => write!(f, "swamp"),
            InterestingTerrainType::Wall => write!(f, "wall"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisualObject {
    InterestingTerrain {
        x: u32,
        y: u32,
        ty: InterestingTerrainType,
    },
    Flag(Flag),
    RoomObject(Arc<KnownRoomObject>),
}

impl VisualObject {
    pub fn x(&self) -> u32 {
        match self {
            VisualObject::InterestingTerrain { x, .. } => *x,
            VisualObject::Flag(x) => x.x,
            VisualObject::RoomObject(x) => x.x(),
        }
    }

    pub fn y(&self) -> u32 {
        match self {
            VisualObject::InterestingTerrain { y, .. } => *y,
            VisualObject::Flag(x) => x.y,
            VisualObject::RoomObject(x) => x.y(),
        }
    }

    pub fn to_symbol(&self) -> &'static str {
        match self {
            VisualObject::InterestingTerrain {
                ty: InterestingTerrainType::Swamp,
                ..
            } => "⌇",
            VisualObject::InterestingTerrain {
                ty: InterestingTerrainType::Wall,
                ..
            } => "█",
            VisualObject::Flag(_) => "F",
            VisualObject::RoomObject(obj) => match &**obj {
                KnownRoomObject::ConstructionSite(..) => "△",
                KnownRoomObject::Container(..) => "▫",
                KnownRoomObject::Controller(..) => "C",
                KnownRoomObject::Creep(..) => "●",
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
                KnownRoomObject::Rampart(..) => "▒",
                KnownRoomObject::Resource(..) => "▪",
                KnownRoomObject::Road(..) => "╬",
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

    pub fn multiple_to_symbol(items: &[VisualObject]) -> &'static str {
        if let Some(obj) = items.last() {
            obj.to_symbol()
        } else {
            " "
        }
    }
}

impl PartialEq for VisualObject {
    fn eq(&self, other: &VisualObject) -> bool {
        use VisualObject::*;
        match (self, other) {
            (
                InterestingTerrain {
                    ty: ty1,
                    x: x1,
                    y: y1,
                },
                InterestingTerrain {
                    ty: ty2,
                    x: x2,
                    y: y2,
                },
            ) => ty1 == ty2 && x1 == x2 && y1 == y2,
            (Flag(a), Flag(b)) => a == b,
            (RoomObject(a), RoomObject(b)) => {
                RoomObjectType::of(a) == RoomObjectType::of(b) && a.id() == b.id()
            }
            (..) => false,
        }
    }
}

impl Eq for VisualObject {}

impl PartialOrd for VisualObject {
    fn partial_cmp(&self, other: &VisualObject) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VisualObject {
    fn cmp(&self, other: &VisualObject) -> Ordering {
        use VisualObject::*;
        match (self, other) {
            (
                InterestingTerrain {
                    ty: ty1,
                    x: x1,
                    y: y1,
                },
                InterestingTerrain {
                    ty: ty2,
                    x: x2,
                    y: y2,
                },
            ) => ty1.cmp(ty2).then(x1.cmp(x2)).then(y1.cmp(y2)),
            (InterestingTerrain { .. }, _) => Ordering::Less,
            (_, InterestingTerrain { .. }) => Ordering::Greater,
            (Flag(a), Flag(b)) => a.name.cmp(&b.name),
            (Flag(_), _) => Ordering::Less,
            (_, Flag(_)) => Ordering::Greater,
            (RoomObject(a), RoomObject(b)) => RoomObjectType::of(a)
                .cmp(&RoomObjectType::of(b))
                .then_with(|| a.id().cmp(b.id())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualRoom {
    pub last_update_time: Option<u32>,
    pub room_id: RoomId,
    pub objs: Array<Vec<VisualObject>, Ix2>,
    pub rendered_rows: Option<Vec<String>>,
    pub users: HashMap<String, Arc<RoomUserInfo>>,
}

impl VisualRoom {
    fn new(
        last_update_time: Option<u32>,
        room_id: RoomId,
        users: HashMap<String, Arc<RoomUserInfo>>,
    ) -> Self {
        VisualRoom {
            last_update_time,
            room_id,
            objs: Array::from_elem((50, 50), Vec::new()),
            rendered_rows: None,
            users,
        }
    }
}

impl VisualRoom {
    fn push_top(&mut self, item: VisualObject) {
        self.objs
            .get_mut([item.x() as usize, item.y() as usize])
            .expect("expected all objects to have valid coordinates (0-49)")
            .push(item);
    }

    fn render_rows(&mut self) {
        let rows = self
            .objs
            .gencolumns()
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|pos_objs| VisualObject::multiple_to_symbol(&*pos_objs))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        self.rendered_rows = Some(rows);
    }
}
