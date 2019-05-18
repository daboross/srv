//! Room state
use std::collections::{hash_map::Entry, HashMap};

use log::{debug, info};
use screeps_api::{
    websocket::{
        types::room::flags::Flag, types::room::objects::KnownRoomObject, Channel, ChannelUpdate,
        RoomUpdate, RoomUserInfo, ScreepsMessage, SockjsMessage,
    },
    Api, MyInfo, RoomName, TokenStorage,
};

#[derive(Clone, Debug, Default)]
pub struct Room {
    objects: HashMap<String, KnownRoomObject>,
    flags: Vec<Flag>,
    users: HashMap<String, RoomUserInfo>,
}

impl Room {
    pub fn update(&mut self, update: RoomUpdate) -> Result<(), serde_json::Error> {
        debug!("updating objects");
        for (id, data) in update.objects.into_iter() {
            debug!(
                "updating {} with data:\n\t{}",
                id,
                serde_json::to_string_pretty(&data).unwrap()
            );
            match self.objects.entry(id) {
                Entry::Occupied(entry) => {
                    entry.into_mut().update(data)?;
                }
                Entry::Vacant(entry) => {
                    entry.insert(serde_json::from_value(data)?);
                }
            }
        }
        debug!("updating flags");
        self.flags = update.flags;

        debug!("updating users");
        for (user_id, data) in update.users.into_iter().flat_map(|x| x) {
            debug!(
                "updating user {} with data:\n\t{}",
                user_id,
                serde_json::to_string_pretty(&data).unwrap()
            );
            match self.users.entry(user_id) {
                Entry::Occupied(entry) => {
                    entry.into_mut().update(serde_json::from_value(data)?);
                }
                Entry::Vacant(entry) => {
                    entry.insert(serde_json::from_value(data)?);
                }
            }
        }

        debug!("update complete");

        Ok(())
    }
}
