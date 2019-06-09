use std::{collections::HashMap, fmt, fmt::Write, sync::Arc};

use screeps_api::websocket::{
    objects::{
        ConstructionSite, Creep, KnownRoomObject, Mineral, Resource, Source, StructureContainer,
        StructureController, StructureExtension, StructureExtractor, StructureKeeperLair,
        StructureLab, StructureLink, StructureNuker, StructureObserver, StructurePortal,
        StructurePowerBank, StructurePowerSpawn, StructureRampart, StructureRoad, StructureSpawn,
        StructureStorage, StructureTerminal, StructureTower, StructureWall, Tombstone,
    },
    resources::ResourceType,
    RoomUserInfo,
};

use crate::room::{RoomObjectType, VisualObject};

pub fn info<T: Info + ?Sized>(thing: &T, state: &InfoInfo) -> String {
    let mut res = String::new();
    thing
        .fmt(&mut res, state)
        .expect("formatting to string should not fail");
    res
}

#[derive(Copy, Clone)]
pub struct InfoInfo<'a> {
    game_time: u32,
    users: &'a HashMap<String, Arc<RoomUserInfo>>,
}

impl<'a> InfoInfo<'a> {
    pub fn new(game_time: u32, users: &'a HashMap<String, Arc<RoomUserInfo>>) -> Self {
        InfoInfo { game_time, users }
    }

    fn username(&self, id: &str) -> Option<&'a str> {
        self.users
            .get(id)
            .and_then(|i| i.username.as_ref())
            .map(AsRef::as_ref)
    }

    fn username_or_fallback<'b>(&self, id: &'b str) -> OptionalUser<'b, 'a> {
        OptionalUser {
            id,
            username: self.username(id),
        }
    }
}

struct OptionalUser<'a, 'b> {
    id: &'a str,
    username: Option<&'b str>,
}

impl fmt::Display for OptionalUser<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.username {
            Some(u) => write!(f, "{}", u),
            None => write!(f, "user {}", self.id),
        }
    }
}

pub trait Info {
    /// Formats self, including a trailing newline
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result;
}

impl<T: Info> Info for [T] {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        for obj in self {
            obj.fmt(out, state)?;
        }
        Ok(())
    }
}

impl<T: Info> Info for Vec<T> {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        // defer to [T]
        self[..].fmt(out, state)
    }
}

impl Info for VisualObject {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        match self {
            VisualObject::InterestingTerrain { ty, .. } => writeln!(out, "terrain: {}", ty),
            VisualObject::Flag(f) => writeln!(out, "flag {}", f.name),
            VisualObject::RoomObject(obj) => obj.fmt(out, state),
        }
    }
}

impl Info for KnownRoomObject {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        match self {
            KnownRoomObject::Source(o) => o.fmt(out, state),
            KnownRoomObject::Mineral(o) => o.fmt(out, state),
            KnownRoomObject::Spawn(o) => o.fmt(out, state),
            KnownRoomObject::Extension(o) => o.fmt(out, state),
            KnownRoomObject::Extractor(o) => o.fmt(out, state),
            KnownRoomObject::Wall(o) => o.fmt(out, state),
            KnownRoomObject::Road(o) => o.fmt(out, state),
            KnownRoomObject::Rampart(o) => o.fmt(out, state),
            KnownRoomObject::KeeperLair(o) => o.fmt(out, state),
            KnownRoomObject::Controller(o) => o.fmt(out, state),
            KnownRoomObject::Portal(o) => o.fmt(out, state),
            KnownRoomObject::Link(o) => o.fmt(out, state),
            KnownRoomObject::Storage(o) => o.fmt(out, state),
            KnownRoomObject::Tower(o) => o.fmt(out, state),
            KnownRoomObject::Observer(o) => o.fmt(out, state),
            KnownRoomObject::PowerBank(o) => o.fmt(out, state),
            KnownRoomObject::PowerSpawn(o) => o.fmt(out, state),
            KnownRoomObject::Lab(o) => o.fmt(out, state),
            KnownRoomObject::Terminal(o) => o.fmt(out, state),
            KnownRoomObject::Container(o) => o.fmt(out, state),
            KnownRoomObject::Nuker(o) => o.fmt(out, state),
            KnownRoomObject::Tombstone(o) => o.fmt(out, state),
            KnownRoomObject::Creep(o) => o.fmt(out, state),
            KnownRoomObject::Resource(o) => o.fmt(out, state),
            KnownRoomObject::ConstructionSite(o) => o.fmt(out, state),
            other => {
                let ty = RoomObjectType::of(&other);
                let ty = string_morph::to_kebab_case(&format!("{:?}", ty));
                writeln!(out, "{} {}", ty, other.id())?;
                Ok(())
            }
        }
    }
}

impl Info for Source {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "source:")?;
        fmt_id(out, &self.id)?;
        fmt_energy(out, self.energy, self.energy_capacity as i32)?;
        if self.energy != self.energy_capacity {
            if let Some(gen_time) = self.next_regeneration_time {
                writeln!(out, "  regen in: {}", gen_time - state.game_time)?;
            }
        }
        Ok(())
    }
}

impl Info for Mineral {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(
            out,
            "mineral: {} {}",
            self.mineral_amount,
            kebab_of_debug(self.mineral_type)
        )?;
        fmt_id(out, &self.id)?;
        Ok(())
    }
}

impl Info for StructureSpawn {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "spawn {}:", self.room)?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        Ok(())
    }
}

impl Info for StructureExtension {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "extension:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        Ok(())
    }
}

impl Info for StructureExtractor {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_optional_user_prefix(out, &self.user, state)?;
        writeln!(out, "extractor:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        Ok(())
    }
}

impl Info for StructureWall {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "wall:")?;
        fmt_id(out, &self.id)?;
        fmt_hits_inf(out, self.hits, self.hits_max)?;
        Ok(())
    }
}

impl Info for StructureRoad {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "road:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        writeln!(out, " decay in: {}", self.next_decay_time - state.game_time)?;
        Ok(())
    }
}

impl Info for StructureRampart {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "rampart:")?;
        fmt_id(out, &self.id)?;
        fmt_hits_inf(out, self.hits, self.hits_max)?;
        writeln!(out, " decay in: {}", self.next_decay_time - state.game_time)?;
        if self.public {
            writeln!(out, " --public--")?;
        } else {
            writeln!(out, " --private--")?;
        }
        Ok(())
    }
}

impl Info for StructureKeeperLair {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "keeper lair:")?;
        fmt_id(out, &self.id)?;
        if let Some(spawn_time) = self.next_spawn_time {
            writeln!(out, " spawning in: {}", spawn_time - state.game_time)?;
        }
        Ok(())
    }
}

impl Info for StructureController {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_optional_user_prefix(out, &self.user, state)?;
        writeln!(out, "controller:")?;
        fmt_id(out, &self.id)?;
        if let Some(sign) = &self.sign {
            // TODO: wrap text?
            writeln!(out, " {}", sign.text)?;
            write!(out, " - {}", state.username_or_fallback(&sign.user_id))?;

            // TODO: real time?
            writeln!(out, " - {} ticks ago", state.game_time - sign.game_time_set)?;
        }
        if self.user.is_some() {
            writeln!(out, " level: {}", self.level)?;
            if let Some(required) = self.progress_required() {
                let progress_percent =
                    (required as f64 - self.progress as f64) / required as f64 * 100.0;
                writeln!(out, " progress: %{:.2}", progress_percent)?;
            }
            // TODO: red text for almost downgraded
            if let Some(time) = self.downgrade_time {
                // TODO: see what this data looks like?
                writeln!(out, " downgrade time: {}", time)?;
            }
            // TODO: only apply this to owned controllers, maybe?
            writeln!(out, " safemode:")?;
            if let Some(end_time) = self.safe_mode {
                if state.game_time < end_time {
                    writeln!(out, "  --safe mode active--")?;
                    writeln!(out, "  ends in: {}", end_time - state.game_time)?;
                }
            }
            writeln!(out, "  available: {}", self.safe_mode_available)?;
            if self.safe_mode_cooldown > state.game_time {
                writeln!(
                    out,
                    "  activation cooldown: {}",
                    self.safe_mode_cooldown - state.game_time
                )?;
            }
        }
        if let Some(reservation) = &self.reservation {
            if reservation.end_time > state.game_time {
                writeln!(
                    out,
                    " reserved by {}",
                    state.username_or_fallback(&reservation.user)
                )?;
                writeln!(out, "  ends in {}", reservation.end_time - state.game_time)?;
            }
        }
        Ok(())
    }
}

impl Info for StructurePortal {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_id(out, &self.id)?;
        writeln!(
            out,
            "portal -> {},{} in {}",
            self.destination.x, self.destination.y, self.destination.room
        )?;
        if let Some(date) = self.unstable_date {
            // TODO: figure out time formatting
            writeln!(out, " stable (decay time is in days)")?;
            writeln!(out, "  (time formatting unimplemented)")?;
        }
        if let Some(time) = self.decay_time {
            writeln!(out, " decays in {} ticks", time - state.game_time)?;
        }
        Ok(())
    }
}

impl Info for StructureLink {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "link:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        if self.cooldown != 0 {
            writeln!(out, " cooldown: {}", self.cooldown)?;
        }
        Ok(())
    }
}

impl Info for StructureStorage {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "storage:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        Ok(())
    }
}

impl Info for StructureTower {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "tower:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        Ok(())
    }
}

impl Info for StructureObserver {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "observer:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        if let Some(name) = self.observed {
            writeln!(out, " observing {}", name)?;
        }
        Ok(())
    }
}

impl Info for StructurePowerBank {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "power bank:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        writeln!(out, " power: {}", self.power)?;
        writeln!(out, " decay in: {}", self.decay_time - state.game_time)?;
        Ok(())
    }
}

impl Info for StructurePowerSpawn {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "power spawn:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        writeln!(out, " power: {}/{}", self.power, self.power_capacity)?;
        Ok(())
    }
}

impl Info for StructureLab {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "lab:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity)?;
        match self.mineral_type {
            Some(ty) => {
                writeln!(
                    out,
                    " {}: {}/{}",
                    kebab_of_debug(ty),
                    self.mineral_amount,
                    self.mineral_capacity
                )?;
            }
            None => {
                writeln!(
                    out,
                    " minerals: {}/{}",
                    self.mineral_amount, self.mineral_capacity
                )?;
            }
        }
        if self.cooldown != 0 {
            writeln!(out, " cooldown: {}", self.cooldown)?;
        }
        Ok(())
    }
}

// TODO: Terminal/Container/Nuker/Tombstone/Resource/ConstructionSite

impl Info for StructureTerminal {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "terminal:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        if self.capacity > 0 {
            writeln!(
                out,
                " capacity: {}/{}",
                self.resources().map(|(_, amt)| amt).sum::<i32>(),
                self.capacity
            )?;
            format_object_contents(out, self.resources())?;
        }
        Ok(())
    }
}

impl Info for StructureContainer {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "container:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        writeln!(out, " decay in: {}", self.next_decay_time - state.game_time)?;
        if self.capacity > 0 {
            writeln!(
                out,
                " capacity: {}/{}",
                self.resources().map(|(_, amt)| amt).sum::<i32>(),
                self.capacity
            )?;
            format_object_contents(out, self.resources())?;
        }
        Ok(())
    }
}

impl Info for StructureNuker {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "nuker:")?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        fmt_disabled(out, self.disabled)?;
        fmt_energy(out, self.energy, self.energy_capacity as i32)?;
        writeln!(out, " ghodium: {}/{}", self.ghodium, self.ghodium_capacity)?;
        if self.cooldown_time < state.game_time {
            writeln!(out, "--ready--")?;
        } else {
            writeln!(out, " cooldown: {}", self.cooldown_time - state.game_time)?;
        }
        Ok(())
    }
}

impl Info for Tombstone {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "tombstone:")?;
        fmt_id(out, &self.id)?;
        writeln!(out, " creep:")?;
        writeln!(out, "  id: {}", self.creep_id)?;
        writeln!(out, "  name: {}", self.creep_name)?;
        writeln!(out, "  ttl: {}", self.creep_ticks_to_live)?;
        writeln!(out, " died: {}", state.game_time - self.death_time)?;
        writeln!(out, " decay in: {}", self.decay_time - state.game_time)?;
        writeln!(out, " contents:")?;
        format_object_contents(out, self.resources())?;
        Ok(())
    }
}

impl Info for Creep {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        fmt_user_prefix(out, &self.user, state)?;
        writeln!(out, "creep {}:", self.name)?;
        fmt_id(out, &self.id)?;
        fmt_hits(out, self.hits, self.hits_max)?;
        if self.fatigue != 0 {
            writeln!(out, " fatigue: {}", self.fatigue)?;
        }
        if let Some(age_time) = self.age_time {
            writeln!(out, " life: {}", age_time - state.game_time)?;
        }
        if self.capacity > 0 {
            writeln!(
                out,
                " capacity: {}/{}",
                self.carry_contents().map(|(_, amt)| amt).sum::<i32>(),
                self.capacity
            )?;
            format_object_contents(out, self.carry_contents())?;
        }

        Ok(())
    }
}

impl Info for Resource {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(out, "dropped {}:", kebab_of_debug(self.resource_type))?;
        writeln!(out, " amount: {}", self.amount)?;
        Ok(())
    }
}
impl Info for ConstructionSite {
    fn fmt<W: Write>(&self, out: &mut W, state: &InfoInfo) -> fmt::Result {
        writeln!(
            out,
            "construction site for {}",
            kebab_of_debug(&self.structure_type)
        )?;
        Ok(())
    }
}

fn fmt_optional_user_prefix<W: Write>(
    out: &mut W,
    user_id: &Option<String>,
    state: &InfoInfo,
) -> fmt::Result {
    if let Some(user_id) = user_id {
        fmt_user_prefix(out, user_id, state)?;
    }

    Ok(())
}

fn fmt_user_prefix<W: Write>(out: &mut W, user_id: &str, state: &InfoInfo) -> fmt::Result {
    write!(out, "[{}] ", state.username_or_fallback(user_id))
}

fn fmt_id<W: Write>(out: &mut W, id: &str) -> fmt::Result {
    writeln!(out, " id: {}", id)
}

fn fmt_hits<W: Write>(out: &mut W, hits: i32, hits_max: i32) -> fmt::Result {
    writeln!(out, " hits: {}/{}", hits, hits_max)
}

fn fmt_hits_inf<W: Write>(out: &mut W, hits: i32, hits_max: i32) -> fmt::Result {
    if f64::from(hits) > f64::from(hits_max) * 0.9 {
        fmt_hits(out, hits, hits_max)
    } else {
        writeln!(out, "hits: {}", hits)
    }
}

fn fmt_energy<W: Write>(out: &mut W, energy: i32, energy_capacity: i32) -> fmt::Result {
    writeln!(out, " energy: {}/{}", energy, energy_capacity)
}

fn fmt_disabled<W: Write>(out: &mut W, disabled: bool) -> fmt::Result {
    if disabled {
        writeln!(out, " --disabled--")?;
    }
    Ok(())
}

fn kebab_of_debug<T: fmt::Debug>(item: T) -> String {
    string_morph::to_kebab_case(&format!("{:?}", item))
}

fn format_object_contents<W, T>(out: &mut W, contents: T) -> fmt::Result
where
    W: fmt::Write,
    T: Iterator<Item = (ResourceType, i32)>,
{
    for (ty, amount) in contents {
        if amount > 0 {
            writeln!(out, "  {}: {}", kebab_of_debug(ty), amount)?;
        }
    }
    Ok(())
}
