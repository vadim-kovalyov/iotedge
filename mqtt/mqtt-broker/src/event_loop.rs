// - The idea to pass Semaphore permit to the SessionState::waiting_to_be_sent queue did not work, since SessionState is serializable (for persistence). The alternative is to impl custom serialization, which (combined with other changes) is already a bit too much.

// Now I started working on the main idea of changing broker threading model. I studied all moving parts and here is the plan as I see it.

// On server start:
// - collect transport[].incoming() into `incomings: Vec<_>`
// - start event loop
// - event loop:
//     - for each i in incomings do i.poll_next():
//         - if ready - add new connection to Vec<Connection>, Connection will have two parts - Sink(outgoing_connections) and Stream (incoming_connections)
//         - else continue
//     - for each c in connections do ic.poll_next() 
//         - this will poll associated incoming_connection or advance Connection state machine.
//         - if packet is ready as a result of poll_next() - call broker.process_packet(packet) - non blocking call.
//         - else continue
//     - for each oc in outgoing_connections do connection_handle.peek()
//         - if Ok(packet) - send to the socket.
//             - if sent,
//         - if Err(Empty) - continue
//     - broker.poll() - non-blocking call to process session queues. See below.
    
// Connection will be a state machine
//     - WaitingForConnect(codec, authenticator)
//     - WaitingForAuth(codec, authenticator)
//     - Connected(codec)

// broker.process_packet(packet) will do:
//     - the main broker logic (as it is right now)
//     - the only exception is that it is only one iteration.
//     - as a result of broker.process_packet(), a packet must be:
//         - put into connection_handle of matching sessions.
//         - put into waiting_to_be_sent queue of matching sessions.
//         - dropped.

// broker.poll() will do the following:
//     - for each s in sessions, 
//         - if waiting_to_be_sent messages.len() > 0 and allowed_to_send()
//             - send to connection_handle




use std::net::SocketAddr;
use std::sync::Arc;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::future::{select, Either};
use futures_util::pin_mut;
use futures_util::sink::{Sink, SinkExt};
use futures_util::stream::{Stream, StreamExt};
use lazy_static::lazy_static;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::{
    net::ToSocketAddrs,
    sync::mpsc::{self, UnboundedReceiver},
};
use tokio_io_timeout::TimeoutStream;
use tokio_util::codec::Framed;
use tracing::{debug, info, span, trace, warn, Level};
use tracing_futures::Instrument;
use uuid::Uuid;

use mqtt3::proto::{self, DecodeError, EncodeError, Packet, PacketCodec};
use mqtt_broker_core::{
    auth::{Authenticator, Authorizer, Certificate, Credentials},
    ClientId,
};

#[cfg(feature = "edgehub")]
use mqtt_edgehub::translation::{
    translate_incoming_publish, translate_incoming_subscribe, translate_incoming_unsubscribe,
    translate_outgoing_publish,
};

use crate::broker::BrokerHandle;
use crate::transport::{GetPeerCertificate, Incoming};
use crate::{
    Auth, Broker, ClientEvent, ConnReq, ConnectionHandle, Error, Message, Publish, TransportBuilder,
};
use futures::Future;

lazy_static! {
    static ref DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
}

const KEEPALIVE_MULT: f32 = 1.5;

pub struct EventLoop<F, N, Z>
where
    F: Future<Output = ()> + Unpin,
    N: Authenticator + Send + Sync + 'static,
    Z: Authorizer + Send + Sync + 'static,
{
    incomings: Vec<Incoming>,
    shutdown_signal: F,
    authenticator: Arc<N>,
    broker: Broker<Z>,
}

impl<F, N, Z> EventLoop<F, N, Z>
where
    F: Future<Output = ()> + Unpin,
    N: Authenticator + Send + Sync + 'static,
    Z: Authorizer + Send + Sync + 'static,
{
    pub async fn new<A, T>(
        broker: Broker<Z>,
        transports: T,
        shutdown_signal: F,
        authenticator: Arc<N>,
    ) -> Result<Self, Error>
    where
        A: ToSocketAddrs,
        T: IntoIterator<Item = TransportBuilder<A>>,
    {
        let mut incomings = Vec::new();

        for transport in transports {
            let io = transport.build().await?;
            let incoming = io.incoming();
            incomings.push(incoming);
        }

        Ok(EventLoop {
            incomings,
            shutdown_signal,
            authenticator,
            broker,
        })
    }
}

impl<F, N, Z> Future for EventLoop<F, N, Z>
where
    F: Future<Output = ()> + Unpin,
    N: Authenticator + Send + Sync + 'static,
    Z: Authorizer + Send + Sync + 'static,
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        for incoming in self.incomings {
            match incoming.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(stream))) => {
                    // let peer = stream.peer_addr();

                    // if let Some(peer) = peer {
                        
                    // }

                    let certificate = stream.peer_certificate().unwrap();

                    let mut timeout = TimeoutStream::new(stream);
                    timeout.set_read_timeout(Some(*DEFAULT_TIMEOUT));
                    timeout.set_write_timeout(Some(*DEFAULT_TIMEOUT));

                    let mut codec = Framed::new(timeout, PacketCodec::default());

                    let authenticator = self.authenticator.clone();
                    continue;
                }
                Poll::Ready(_) => {
                    continue;
                }
                Poll::Pending => {
                    continue;
                }
            }
        }

        Poll::Pending
    }
}

// pub async fn start<A, F, I, N>(
//     transports: I,
//     shutdown_signal: F,
//     authenticator: Arc<N>,
// ) -> Result<(), Error>
// where
//     A: ToSocketAddrs,
//     F: Future<Output = ()> + Unpin,
//     I: IntoIterator<Item = TransportBuilder<A>>,
//     N: Authenticator + Send + Sync + 'static,
// {
//     let incomings = Vec::new();

//     for transport in transports {
//         let io = transport.build().await?;
//         let incoming = io.incoming();
//         incomings.push(incoming);
//     }

//     loop {
//         for incoming in incomings {
//             incoming.poll_next_unpin();
//         }
//     }

//     // let addr = io.local_addr()?;
//     // let span = span!(Level::INFO, "server", listener=%addr);
//     // let _enter = span.enter();

//     // let certificate = io.peer_certificate()?;

//     // let mut timeout = TimeoutStream::new(io);
//     // timeout.set_read_timeout(Some(*DEFAULT_TIMEOUT));
//     // timeout.set_write_timeout(Some(*DEFAULT_TIMEOUT));

//     // let mut codec = Framed::new(timeout, PacketCodec::default());

//     Ok(())
// }

/// Handles packet processing for a single connection.
///
/// Receives a source of packets and a handle to the Broker.
/// Starts two tasks (sending and receiving)
pub async fn process<I, N>(
    io: I,
    remote_addr: SocketAddr,
    mut broker_handle: BrokerHandle,
    authenticator: Arc<N>,
) -> Result<(), Error>
where
    I: AsyncRead + AsyncWrite + GetPeerCertificate<Certificate = Certificate> + Unpin,
    N: Authenticator + Send + Sync + 'static,
{
    let certificate = io.peer_certificate()?;

    let mut timeout = TimeoutStream::new(io);
    timeout.set_read_timeout(Some(*DEFAULT_TIMEOUT));
    timeout.set_write_timeout(Some(*DEFAULT_TIMEOUT));

    let mut codec = Framed::new(timeout, PacketCodec::default());

    // [MQTT-3.1.0-1] - After a Network Connection is established by a Client to a Server,
    // the first Packet sent from the Client to the Server MUST be a CONNECT Packet.
    //
    // We need to handle the first CONNECT packet here (instead of in the broker state machine)
    // so that we can get and cache the client_id for use with other packets.
    // The broker state machine will also have to handle not receiving a connect packet first
    // to keep the state machine correct.

    match codec.next().await {
        Some(Ok(Packet::Connect(connect))) => {
            let client_id = client_id(&connect.client_id);
            let (sender, events) = mpsc::unbounded_channel();
            let connection_handle = ConnectionHandle::from_sender(sender);
            let span = span!(Level::INFO, "connection", client_id=%client_id, remote_addr=%remote_addr, connection=%connection_handle);

            // async block to attach instrumentation context
            async {
                info!("new client connection");
                debug!("received CONNECT: {:?}", connect);

                // [MQTT-3.1.2-24] - If the Keep Alive value is non-zero and
                // the Server does not receive a Control Packet from the
                // Client within one and a half times the Keep Alive time
                // period, it MUST disconnect the Network Connection to the
                // Client as if the network had failed.
                let keep_alive = connect.keep_alive.mul_f32(KEEPALIVE_MULT);
                if keep_alive == Duration::from_secs(0) {
                    debug!("received 0 length keepalive from client. disabling keepalive timeout");
                    codec.get_mut().set_read_timeout(None);
                } else {
                    debug!("using keepalive timeout of {:?}", keep_alive);
                    codec.get_mut().set_read_timeout(Some(keep_alive));
                }

                // [MQTT-3.1.4-3] - The Server MAY check that the contents of the CONNECT
                // Packet meet any further restrictions and MAY perform authentication
                // and authorization checks. If any of these checks fail, it SHOULD send an
                // appropriate CONNACK response with a non-zero return code as described in
                // section 3.2 and it MUST close the Network Connection.
                let credentials = certificate.map_or(
                    Credentials::Password(
                        connect.password.clone(),
                    ),
                    Credentials::ClientCertificate,
                );

                let auth = match authenticator.authenticate(connect.username.clone(), credentials).await {
                    Ok(Some(auth_id)) => Auth::Identity(auth_id),
                    Ok(None) => Auth::Unknown,
                    Err(e) => {
                        warn!(message = "error authenticating client: {}", error = %e);
                        Auth::Failure
                    }
                };

                let req = ConnReq::new(client_id.clone(), connect, auth, connection_handle);
                let event = ClientEvent::ConnReq(req);
                let message = Message::Client(client_id.clone(), event);
                broker_handle.send(message).await?;

                // Start up the processing tasks
                let (outgoing, incoming) = codec.split();
                let incoming_task =
                    incoming_task(client_id.clone(), incoming, broker_handle.clone());
                let outgoing_task = outgoing_task(client_id.clone(), events, outgoing, broker_handle.clone());
                pin_mut!(incoming_task);
                pin_mut!(outgoing_task);

                match select(incoming_task, outgoing_task).await {
                    Either::Left((Ok(()), out)) => {
                        debug!("incoming_task finished with ok. waiting for outgoing_task to complete...");

                        if let Err((mut recv, e)) = out.await {
                            debug!(message = "outgoing_task finished with an error. draining message receiver for connection...", %e);
                            while let Some(message) = recv.recv().await {
                                trace!("dropping {:?}", message);
                            }
                            debug!("message receiver draining completed.");
                        }
                        debug!("outgoing_task completed.");
                    }
                    Either::Left((Err(e), out)) => {
                        // incoming packet stream completed with an error
                        // send a DropConnection request to the broker and wait for the outgoing
                        // task to drain
                        debug!(message = "incoming_task finished with an error. sending drop connection request to broker", error=%e);
                        let msg = Message::Client(client_id.clone(), ClientEvent::DropConnection);
                        broker_handle.send(msg).await?;

                        debug!("waiting for outgoing_task to complete...");
                        if let Err((mut recv, e)) = out.await {
                            debug!(message = "outgoing_task finished with an error. draining message receiver for connection...", %e);
                            while let Some(message) = recv.recv().await {
                                trace!("dropping {:?}", message);
                            }
                            debug!("message receiver draining completed.");
                        }
                        debug!("outgoing_task completed.");
                    }
                    Either::Right((Ok(()), inc)) => {
                        drop(inc);
                        debug!("outgoing finished with ok")
                    }
                    Either::Right((Err((mut recv, e)), inc)) => {
                        // outgoing task failed with an error.
                        // drop the incoming packet processing
                        // Notify the broker that the connection is gone, drain the receiver, and
                        // close the connection

                        drop(inc);

                        debug!(message = "outgoing_task finished with an error. notifying the broker to remove the connection", %e);
                        let msg = Message::Client(client_id.clone(), ClientEvent::CloseSession);
                        broker_handle.send(msg).await?;

                        debug!("draining message receiver for connection...");
                        while let Some(message) = recv.recv().await {
                            trace!("dropping {:?}", message);
                        }
                        debug!("message receiver draining completed.");
                    }
                }

                info!("closing connection");
                Ok(())
            }
                .instrument(span)
                .await
        }
        Some(Ok(packet)) => Err(Error::NoConnect(packet)),
        Some(Err(e)) => Err(e.into()),
        None => Err(Error::NoPackets),
    }
}

async fn incoming_task<S>(
    client_id: ClientId,
    mut incoming: S,
    mut broker: BrokerHandle,
) -> Result<(), Error>
where
    S: Stream<Item = Result<Packet, DecodeError>> + Unpin,
{
    debug!("incoming_task start");
    while let Some(maybe_packet) = incoming.next().await {
        match maybe_packet {
            Ok(packet) => {
                let event = match packet {
                    Packet::Connect(_) => {
                        // [MQTT-3.1.0-2] - The Server MUST process a second CONNECT Packet
                        // sent from a Client as a protocol violation and disconnect the Client.

                        warn!("CONNECT packet received on an already established connection, dropping connection due to protocol violation");
                        return Err(Error::ProtocolViolation);
                    }
                    Packet::ConnAck(connack) => ClientEvent::ConnAck(connack),
                    Packet::Disconnect(disconnect) => {
                        let event = ClientEvent::Disconnect(disconnect);
                        let message = Message::Client(client_id.clone(), event);
                        broker.send(message).await?;
                        debug!("disconnect received. shutting down receive side of connection");
                        return Ok(());
                    }
                    Packet::PingReq(ping) => ClientEvent::PingReq(ping),
                    Packet::PingResp(pingresp) => ClientEvent::PingResp(pingresp),
                    Packet::PubAck(puback) => ClientEvent::PubAck(puback),
                    Packet::PubComp(pubcomp) => ClientEvent::PubComp(pubcomp),
                    Packet::Publish(publish) => {
                        #[cfg(feature = "edgehub")]
                        let publish = translate_incoming_publish(&client_id, publish);
                        ClientEvent::PublishFrom(publish)
                    }
                    Packet::PubRec(pubrec) => ClientEvent::PubRec(pubrec),
                    Packet::PubRel(pubrel) => ClientEvent::PubRel(pubrel),
                    Packet::Subscribe(subscribe) => {
                        #[cfg(feature = "edgehub")]
                        let subscribe = translate_incoming_subscribe(&client_id, subscribe);
                        ClientEvent::Subscribe(subscribe)
                    }
                    Packet::SubAck(suback) => ClientEvent::SubAck(suback),
                    Packet::Unsubscribe(unsubscribe) => {
                        #[cfg(feature = "edgehub")]
                        let unsubscribe = translate_incoming_unsubscribe(&client_id, unsubscribe);
                        ClientEvent::Unsubscribe(unsubscribe)
                    }
                    Packet::UnsubAck(unsuback) => ClientEvent::UnsubAck(unsuback),
                };

                let message = Message::Client(client_id.clone(), event);
                broker.send(message).await?;
            }
            Err(e) => {
                warn!(message="error occurred while reading from connection", error=%e);
                return Err(e.into());
            }
        }
    }

    debug!("no more packets. sending DropConnection to broker.");
    let message = Message::Client(client_id.clone(), ClientEvent::DropConnection);
    broker.send(message).await?;
    debug!("incoming_task completing...");
    Ok(())
}

async fn outgoing_task<S>(
    client_id: ClientId,
    mut messages: UnboundedReceiver<Message>,
    mut outgoing: S,
    mut broker: BrokerHandle,
) -> Result<(), (UnboundedReceiver<Message>, Error)>
where
    S: Sink<Packet, Error = EncodeError> + Unpin,
{
    debug!("outgoing_task start");
    while let Some(message) = messages.recv().await {
        debug!("outgoing: {:?}", message);
        let maybe_packet = match message {
            Message::Client(_client_id, event) => match event {
                ClientEvent::ConnReq(_) => None,
                ClientEvent::ConnAck(connack) => Some(Packet::ConnAck(connack)),
                ClientEvent::Disconnect(_) => {
                    debug!("asked to disconnect. outgoing_task completing...");
                    return Ok(());
                }
                ClientEvent::DropConnection => {
                    debug!("asked to drop connection. outgoing_task completing...");
                    return Ok(());
                }
                ClientEvent::PingReq(req) => Some(Packet::PingReq(req)),
                ClientEvent::PingResp(response) => Some(Packet::PingResp(response)),
                ClientEvent::Subscribe(sub) => Some(Packet::Subscribe(sub)),
                ClientEvent::SubAck(suback) => Some(Packet::SubAck(suback)),
                ClientEvent::Unsubscribe(unsub) => Some(Packet::Unsubscribe(unsub)),
                ClientEvent::UnsubAck(unsuback) => Some(Packet::UnsubAck(unsuback)),
                ClientEvent::PublishTo(Publish::QoS12(_id, publish)) => {
                    #[cfg(feature = "edgehub")]
                    let publish = translate_outgoing_publish(publish);
                    Some(Packet::Publish(publish))
                }
                ClientEvent::PublishTo(Publish::QoS0(id, publish)) => {
                    #[cfg(feature = "edgehub")]
                    let publish = translate_outgoing_publish(publish);
                    let result = outgoing.send(Packet::Publish(publish)).await;

                    if let Err(e) = result {
                        warn!(message = "error occurred while writing to connection", error=%e);
                        return Err((messages, e.into()));
                    } else {
                        let message = Message::Client(client_id.clone(), ClientEvent::PubAck0(id));
                        if let Err(e) = broker.send(message).await {
                            warn!(message = "error occurred while sending QoS ack to broker", error=%e);
                            return Err((messages, e));
                        }
                    }
                    None
                }
                ClientEvent::PubAck(puback) => Some(Packet::PubAck(puback)),
                ClientEvent::PubRec(pubrec) => Some(Packet::PubRec(pubrec)),
                ClientEvent::PubRel(pubrel) => Some(Packet::PubRel(pubrel)),
                ClientEvent::PubComp(pubcomp) => Some(Packet::PubComp(pubcomp)),
                event => {
                    warn!("ignoring event for outgoing_task: {:?}", event);
                    None
                }
            },
            Message::System(_event) => None,
        };

        if let Some(packet) = maybe_packet {
            let result = outgoing.send(packet).await;

            if let Err(e) = result {
                warn!(message = "error occurred while writing to connection", error=%e);
                return Err((messages, e.into()));
            }
        }
    }
    debug!("outgoing_task completing...");
    Ok(())
}

fn client_id(client_id: &proto::ClientId) -> ClientId {
    let id = match client_id {
        proto::ClientId::ServerGenerated => Uuid::new_v4().to_string(),
        proto::ClientId::IdWithCleanSession(ref id) => id.to_owned(),
        proto::ClientId::IdWithExistingSession(ref id) => id.to_owned(),
    };
    ClientId::from(id)
}
