use std::thread;

use cursive::CbSink;
use err_ctx::ResultExt;
use futures::{
    channel::mpsc::unbounded,
    compat::{Future01CompatExt, Sink01CompatExt, Stream01CompatExt},
    future::{self, Either},
    stream, FutureExt, Sink, SinkExt, Stream, StreamExt, TryFutureExt, TryStreamExt,
};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use log::{debug, error, info, warn};
use old_futures::stream::Stream as OldStream;
use screeps_api::{
    websocket::{subscribe, unsubscribe, Channel, ChannelUpdate, ScreepsMessage, SockjsMessage},
    Api, MyInfo, RoomName, TokenStorage,
};
use websocket::{ClientBuilder, OwnedMessage};

use crate::{
    config::Config,
    room::{ConnectionState, Room, RoomId},
    ui::{self, CursiveStatePair},
};

pub type Error = Box<::std::error::Error + Send + Sync>;

#[derive(Clone, Debug)]
pub enum Command {
    /// Command sent by net internals indicating that the connection should be re-established.
    Reconnect,
    ChangeRoom(RoomId),
}

pub fn spawn(config: Config, ui: CbSink) {
    thread::spawn(|| {
        let err_ui_sink = ui.clone();
        let res = run(config, ui);

        if let Err(e) = res {
            error!("Network thread error: {0} ({0:?})", e);
            // ignore errors sending error report
            let _ = ui::async_update(&err_ui_sink, |s| s.conn_state(ConnectionState::Error));
            panic!("Error: {:?}", e);
        }
    });
}

fn run(config: Config, ui: CbSink) -> Result<(), Error> {
    Stage1::new(config, ui)?.run();
    Ok(())
}

struct Stage1 {
    config: Config,
    client: Api<HttpsConnector<HttpConnector>>,
    ui: CbSink,
}

#[allow(unused)]
struct ConnIndepState {
    config: Config,
    room_id: RoomId,
    client: Api<HttpsConnector<HttpConnector>>,
    ui: CbSink,
    tokens: TokenStorage,
    user: MyInfo,
    room: Room,
}

struct Connected<Si, St> {
    s: ConnIndepState,
    sink: Si,
    stream: St,
}

impl Stage1 {
    pub fn new(config: Config, ui: CbSink) -> Result<Self, Error> {
        let hyper = hyper::Client::builder().build::<_, hyper::Body>(HttpsConnector::new(1)?);

        let mut client = Api::new(hyper);

        if let Some(u) = &config.server {
            client.set_url(u)?;
        }
        client.set_token(config.auth_token.clone());

        let server = client.url.to_string();
        ui::async_update(&ui, |s| s.server(server))?;

        Ok(Stage1 { config, client, ui })
    }

    pub fn run(self) {
        debug!("stage 1 starting");

        tokio::runtime::current_thread::run(
            self.run_tokio()
                .then(|res| {
                    if let Err(e) = res {
                        error!("Error occurred: {0} ({0:?})", e);
                        panic!("Error occurred: {0} ({0:?})", e);
                    }
                    future::ok(())
                })
                .boxed_local()
                .compat(),
        );
    }

    async fn run_tokio(self) -> Result<(), Error> {
        use screeps_api::{
            websocket::{connecting::transform_url, *},
            DEFAULT_OFFICIAL_API_URL,
        };
        let tokens = self.client.token_storage().clone();

        // info.user_id allows subscribing to messages.
        let user = self.client.my_info()?.compat().await?;

        let ui_user = user.clone();
        ui::async_update(&self.ui, |s| s.user(ui_user))?;

        let (shard, room) = match (self.config.shard.as_ref(), self.config.room.as_ref()) {
            (shard, Some(room)) => (shard.cloned(), room.clone()),
            (Some(shard), None) => {
                let room_name = self
                    .client
                    .shard_start_room(shard)?
                    .compat()
                    .await?
                    .room_name;
                let room_name = RoomName::new(&room_name).map_err(|e| e.into_owned())?;
                (Some(shard.clone()), room_name)
            }
            (None, None) => {
                let start_room = self.client.world_start_room()?.compat().await?;
                let room_name = RoomName::new(&start_room.room_name).map_err(|e| e.into_owned())?;
                (start_room.shard, room_name)
            }
        };

        let room_id = RoomId::new(shard, room);

        debug!("starting at room {}", room_id);

        let terrain = self
            .client
            .room_terrain(room_id.shard.as_ref(), room_id.room_name.to_string())
            .compat()
            .await
            .with_ctx(|_| format!("fetching {} terrain", room_id))?;

        debug!("successfully authenticated as {}", user.username);

        let ws_url = self
            .config
            .server
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or(DEFAULT_OFFICIAL_API_URL);

        let ws_url = transform_url(ws_url).ctx("parsing API url")?;

        let room = Room::new(room_id.clone(), terrain);

        let (cmd_send, cmd_recv) = unbounded();

        ui::async_update(&self.ui, |s| s.command_sender(cmd_send))?;

        let mut s = ConnIndepState {
            config: self.config,
            client: self.client,
            ui: self.ui,
            room_id,
            tokens,
            user,
            room,
        };

        let mut cmd_recv = cmd_recv.map(|cmd| Ok(Either::Right(cmd)));

        loop {
            let (conn, _) = ClientBuilder::from_url(&ws_url)
                .async_connect(None)
                .compat()
                .await?;

            let (sink, stream) = conn.split();
            let mut sink = sink.sink_compat().sink_map_err(Error::from);
            let stream = stream.compat().map_err(Error::from);

            // If we didn't have this, then the loop over this stream would just be waiting for commands
            // after the network stream stops. This makes sure that if the network stream is disconnected,
            // then we immediately get a 'Reconnect' message after that.
            let stream = stream
                .map(|res| res.map(Either::Left))
                .chain(stream::once(future::ok(Either::Right(Command::Reconnect))));

            // Listen to both the network stream and our commands
            let stream = stream::select(stream, cmd_recv);

            s.update_ui(|s| s.conn_state(ConnectionState::Authenticating))?;

            sink.send(OwnedMessage::Text(authenticate(&s.tokens.get().unwrap())))
                .await?;
            sink.send(OwnedMessage::Text(subscribe(&Channel::room_detail(
                s.room_id.room_name,
                s.room_id.shard.as_ref(),
            ))))
            .await?;

            let mut conn = Connected { s, sink, stream };
            debug!("stage 1 handing off");
            conn.run().await?;
            debug!("stage 2 ended, stage 1 reconnecting");
            // recapture state
            s = conn.s;
            cmd_recv = conn.stream.into_inner().1;

            s.update_ui(|s| s.conn_state(ConnectionState::Disconnected))?;
        }
    }
}

impl ConnIndepState {
    pub fn update_ui<F: FnOnce(&mut CursiveStatePair) + Send + 'static>(
        &self,
        func: F,
    ) -> Result<(), Error> {
        ui::async_update(&self.ui, func)
    }
}

impl<Si, St> Connected<Si, St>
where
    Si: Sink<OwnedMessage, SinkError = Error> + Unpin,
    St: Stream<Item = Result<Either<OwnedMessage, Command>, Error>> + Unpin,
{
    async fn run(&mut self) -> Result<(), Error> {
        debug!("stage 2 main loop starting");
        while let Some(msg) = self.stream.try_next().await? {
            match msg {
                Either::Left(OwnedMessage::Text(string)) => {
                    info!("handling string:\n{}", string);
                    let data = SockjsMessage::parse(&string)
                        .with_ctx(|_| format!("parsing sockjs message {:?}", string))?;

                    match data {
                        SockjsMessage::Message(inner) => {
                            self.handle_message(inner).await?;
                        }
                        SockjsMessage::Messages(inners) => {
                            for inner in inners {
                                self.handle_message(inner).await?;
                            }
                        }
                        o => info!("ignoring message: {:?}", o),
                    }
                }
                Either::Left(OwnedMessage::Ping(data)) => {
                    self.sink.send(OwnedMessage::Pong(data)).await?;
                }
                Either::Left(o) => info!("ignoring message: {:?}", o),
                Either::Right(cmd) => {
                    debug!("received command {:?}", cmd);
                    match cmd {
                        Command::Reconnect => return Ok(()),
                        Command::ChangeRoom(new_room) => {
                            self.change_room(new_room).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn change_room(&mut self, room_id: RoomId) -> Result<(), Error> {
        debug!("starting at room {}", room_id);

        let terrain = self
            .s
            .client
            .room_terrain(room_id.shard.as_ref(), room_id.room_name.to_string())
            .compat()
            .await
            .with_ctx(|_| format!("fetching {} terrain", room_id))?;

        let old_room_id = self.s.room_id.clone();

        self.sink
            .send(OwnedMessage::Text(unsubscribe(&Channel::room_detail(
                old_room_id.room_name,
                old_room_id.shard.as_ref(),
            ))))
            .await?;

        self.sink
            .send(OwnedMessage::Text(subscribe(&Channel::room_detail(
                room_id.room_name,
                room_id.shard.as_ref(),
            ))))
            .await?;

        self.s.room_id = room_id.clone();
        self.s.room = Room::new(room_id, terrain);
        Ok(())
    }

    async fn handle_message<'a>(&'a mut self, msg: ScreepsMessage<'a>) -> Result<(), Error> {
        info!("handling message {:#?}", msg);
        match msg {
            ScreepsMessage::AuthFailed => return Err("authentication failed".into()),
            ScreepsMessage::AuthOk { new_token } => {
                self.s
                    .update_ui(|s| s.conn_state(ConnectionState::Connected))?;
                self.s.tokens.set(new_token);
            }
            ScreepsMessage::ChannelUpdate {
                update:
                    ChannelUpdate::RoomDetail {
                        room_name,
                        shard_name,
                        update,
                    },
            } => {
                info!("inside branch");
                let update_id = RoomId::new(shard_name, room_name);
                if update_id != self.s.room_id {
                    warn!(
                        "received update for wrong room: expected {}, found {}",
                        self.s.room_id, update_id
                    );
                    return Ok(());
                }

                info!("running update");
                self.s
                    .room
                    .update(update)
                    .with_ctx(|_| format!("handling room update for {}", update_id))?;
                info!("update success");
                info!("room: {:?}", self.s.room);
                let visual = self.s.room.visualize();
                self.s.update_ui(|s| s.room(visual))?;
            }
            other => debug!("ignoring {:?}", other),
        }
        info!("handler done successfully");

        Ok(())
    }
}

/*
fn main() {
    debug!("setting up");

    let config = setup();

    debug!("creating client");

    info!(
        "Logged in as {}, attempting to connect to stream.",
        my_info.username
    );

    let connection = websocket::ClientBuilder::from_url(&ws_url).async_connect(None);

    tokio::runtime::current_thread::run(
        connection
            .then(|result| {
                let (client, _) = result.expect("connecting to server failed");

                let (sink, stream) = client.split();

                sink.send(OwnedMessage::Text(screeps_api::websocket::authenticate(
                    &tokens.get().unwrap(),
                )))
                .and_then(|sink| {
                    let handler = Handler::new(tokens, my_info, config);

                    sink.send_all(
                        stream
                            .and_then(move |data| future::ok(handler.handle_data(data)))
                            .or_else(|err| {
                                warn!("error occurred: {}", err);

                                future::ok::<_, websocket::WebSocketError>(
                                    Box::new(stream::empty())
                                        as Box<
                                            dyn Stream<
                                                Item = OwnedMessage,
                                                Error = websocket::WebSocketError,
                                            >,
                                        >,
                                )
                            })
                            .flatten(),
                    )
                })
            })
            .then(|res| {
                res.unwrap();
                Ok(())
            }),
    );
}

struct Handler {
    tokens: TokenStorage,
    info: screeps_api::MyInfo,
    config: Config,
}

impl Handler {
    fn new(tokens: TokenStorage, info: screeps_api::MyInfo, config: Config) -> Self {
        Handler {
            tokens,
            info,
            config,
        }
    }

    fn handle_data(
        &self,
        data: OwnedMessage,
    ) -> Box<dyn Stream<Item = OwnedMessage, Error = websocket::WebSocketError>> {
        match data {
            OwnedMessage::Text(string) => {
                let data = SockjsMessage::parse(&string)
                    .expect("expected a correct SockJS message, found a parse error.");

                match data {
                    SockjsMessage::Open => debug!("SockJS connection opened"),
                    SockjsMessage::Heartbeat => debug!("SockJS heartbeat."),
                    SockjsMessage::Close { .. } => debug!("SockJS close"),
                    SockjsMessage::Message(message) => {
                        return Box::new(self.handle_parsed_message(message));
                    }
                    SockjsMessage::Messages(messages) => {
                        let results = messages
                            .into_iter()
                            .map(|message| Ok(self.handle_parsed_message(message)))
                            .collect::<Vec<_>>();

                        return Box::new(
                            stream::iter_result::<_, _, websocket::WebSocketError>(results)
                                .flatten(),
                        );
                    }
                }
            }
            OwnedMessage::Binary(data) => warn!("ignoring binary data from websocket: {:?}", data),
            OwnedMessage::Close(data) => warn!("connection closing: {:?}", data),
            OwnedMessage::Ping(data) => {
                return Box::new(stream::once(Ok(OwnedMessage::Pong(data))))
            }
            OwnedMessage::Pong(_) => (),
        }

        Box::new(stream::empty())
    }

    fn handle_parsed_message(
        &self,
        message: screeps_api::websocket::parsing::ScreepsMessage<'_>,
    ) -> Box<dyn Stream<Item = OwnedMessage, Error = websocket::WebSocketError>> {
        match message {
            ScreepsMessage::AuthFailed => panic!("authentication with stored token failed!"),
            ScreepsMessage::AuthOk { new_token } => {
                info!(
                    "authentication succeeded, now authenticated as {}.",
                    self.info.username
                );

                debug!(
                    "replacing stored token with returned token: {:?}",
                    new_token
                );
                // return the token to the token storage in case we need it again - we won't in this
                // example program, but this is a good practice
                self.tokens.set(new_token);

                return Box::new(
                    self.config.subscribe_with(&self.info.user_id).chain(
                        stream::futures_unordered(vec![future::lazy(|| {
                            warn!("subscribed!");
                            future::ok::<_, websocket::WebSocketError>(stream::empty())
                        })])
                        .flatten(),
                    ),
                )
                    as Box<dyn Stream<Item = OwnedMessage, Error = websocket::WebSocketError>>;
            }
            ScreepsMessage::ChannelUpdate { update } => {
                self.handle_update(update);
            }
            ScreepsMessage::ServerProtocol { protocol } => {
                info!("server protocol: {}", protocol);
            }
            ScreepsMessage::ServerTime { time } => {
                info!("server time: {}", time);
            }
            ScreepsMessage::ServerPackage { package } => {
                info!("server package: {}", package);
            }
            ScreepsMessage::Other(other) => {
                warn!("ScreepsMessage::Other: {}", other);
            }
        }

        Box::new(stream::empty())
    }

    fn handle_update(&self, update: ChannelUpdate<'_>) {
        match update {
            ChannelUpdate::UserCpu { user_id, update } => info!("CPU: [{}] {:#?}", user_id, update),
            ChannelUpdate::RoomMapView {
                room_name,
                shard_name,
                update,
            } => {
                info!(
                    "Map View: [{}/{}] {:?}",
                    shard_name.as_ref().map(AsRef::as_ref).unwrap_or("<None>"),
                    room_name,
                    update
                );
            }
            ChannelUpdate::RoomDetail {
                room_name,
                shard_name,
                update,
            } => {
                debug!(
                    "Room Detail: [{}/{}] {:?}",
                    shard_name.as_ref().map(AsRef::as_ref).unwrap_or("<None>"),
                    room_name,
                    update
                );
                info!(
                    "Room {}/{}: {}",
                    shard_name.as_ref().map(AsRef::as_ref).unwrap_or("<None>"),
                    room_name,
                    serde_json::to_string_pretty(&serde_json::Value::Object(
                        update.objects.iter().cloned().collect()
                    ))
                    .expect("expected to_string to succeed on plain map.")
                );
            }
            ChannelUpdate::NoRoomDetail {
                room_name,
                shard_name,
            } => {
                info!(
                    "Room Skipped: {}/{}",
                    shard_name.as_ref().map(AsRef::as_ref).unwrap_or("<None>"),
                    room_name
                );
            }
            ChannelUpdate::UserConsole { user_id, update } => {
                info!("Console: [{}] {:#?}", user_id, update);
            }
            ChannelUpdate::UserCredits { user_id, update } => {
                info!("Credits: [{}] {}", user_id, update);
            }
            ChannelUpdate::UserMessage { user_id, update } => {
                info!("New message: [{}] {:#?}", user_id, update);
            }
            ChannelUpdate::UserConversation {
                user_id,
                target_user_id,
                update,
            } => {
                info!(
                    "Conversation update: [{}->{}] {:#?}",
                    user_id, target_user_id, update
                );
            }
            ChannelUpdate::Other { channel, update } => {
                warn!(
                    "ChannelUpdate::Other: {}\n{}",
                    channel,
                    serde_json::to_string_pretty(&update).expect("failed to serialize json string")
                );
            }
        }
    }
}
*/
