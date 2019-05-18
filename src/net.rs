use std::{borrow::Cow, collections::HashMap, thread};

use cursive::CbSink;
use futures::{
    compat::{Future01CompatExt, Sink01CompatExt, Stream01CompatExt},
    future, stream, Future, FutureExt, Sink, SinkExt, Stream, TryFutureExt, TryStreamExt,
};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use log::{debug, error, info, warn};
use old_futures::stream::Stream as OldStream;
use screeps_api::{
    websocket::{
        types::room::objects::KnownRoomObject, Channel, ChannelUpdate, RoomUpdate, ScreepsMessage,
        SockjsMessage,
    },
    Api, MyInfo, RoomName, TokenStorage,
};
use websocket::{ClientBuilder, OwnedMessage};

use crate::{config::Config, room::Room};

pub type Error = Box<::std::error::Error>;

pub fn spawn(config: Config, ui: CbSink) {
    thread::spawn(|| {
        if let Err(e) = run(config, ui) {
            error!("Error occurred: {0} ({0:?})", e);
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

struct Stage2<Si, St> {
    config: Config,
    client: Api<HttpsConnector<HttpConnector>>,
    ui: CbSink,
    tokens: TokenStorage,
    user: MyInfo,
    sink: Si,
    stream: St,
    room: Room,
}

impl Stage1 {
    pub fn new(config: Config, ui: CbSink) -> Result<Self, Error> {
        let hyper = hyper::Client::builder().build::<_, hyper::Body>(HttpsConnector::new(1)?);

        let mut client = Api::new(hyper);

        if let Some(u) = &config.server {
            client.set_url(u)?;
        }
        client.set_token(config.auth_token.clone());

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

        debug!("successfully authenticated as {}", user.username);

        let ws_url = self
            .config
            .server
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or(DEFAULT_OFFICIAL_API_URL);

        let (conn, _) = ClientBuilder::from_url(&transform_url(ws_url)?)
            .async_connect(None)
            .compat()
            .await?;

        let (sink, stream) = conn.split();
        let mut sink = sink.sink_compat().sink_map_err(Error::from);
        let mut stream = stream.compat().map_err(Error::from);

        sink.send(OwnedMessage::Text(authenticate(&tokens.get().unwrap())))
            .await?;
        sink.send(OwnedMessage::Text(subscribe(&Channel::room_detail(
            self.config.room,
            self.config.shard.as_ref(),
        ))))
        .await?;

        let room = Room::default();

        let next = Stage2 {
            config: self.config,
            client: self.client,
            ui: self.ui,
            tokens,
            user,
            sink,
            stream,
            room,
        };
        debug!("stage 1 handing off");

        next.run().await
    }
}

impl<Si, St> Stage2<Si, St>
where
    Si: Sink<OwnedMessage, SinkError = Error> + Unpin,
    St: Stream<Item = Result<OwnedMessage, Error>> + Unpin,
{
    async fn run(mut self) -> Result<(), Error> {
        debug!("stage 2 main loop starting");
        while let Some(msg) = self.stream.try_next().await? {
            match msg {
                OwnedMessage::Text(string) => {
                    info!("handling string:\n{}", string);
                    let data = SockjsMessage::parse(&string)?;

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
                o => info!("ignoring message: {:?}", o),
            }
        }

        Ok(())
    }

    async fn handle_message<'a>(&'a mut self, msg: ScreepsMessage<'a>) -> Result<(), Error> {
        info!("handling message {:#?}", msg);
        match msg {
            ScreepsMessage::AuthFailed => return Err("authentication failed".into()),
            ScreepsMessage::AuthOk { new_token } => {
                self.tokens.set(new_token);
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
                if room_name != self.config.room || shard_name != self.config.shard {
                    info!("error matching room");
                    return Err(format!(
                        "update for room {:?}:{} unexpected (expected {:?}:{})",
                        shard_name, room_name, self.config.shard, self.config.room
                    )
                    .into());
                }

                info!("running update");
                self.room.update(update)?;
                info!("update success");
                info!("room: {:?}", self.room);
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
