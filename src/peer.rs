//! A peer network node.

#![allow(unused_imports, dead_code, unused_variables, unused_mut)]

use crate::hydrabadger::{Error, Hydrabadger};
use crate::{
    Contribution, InAddr, InternalMessage, NodeId, OutAddr, Uid, WireMessage, WireMessageKind,
    WireMessages, WireRx, WireTx,
};
use hbbft::{crypto::PublicKey, dynamic_honey_badger::Input as HbInput};
use serde::{Deserialize, Serialize};
use tokio::task;
use std::clone;
use std::{
    borrow::Borrow,
    collections::{
        hash_map::{Iter as HashMapIter, Values as HashMapValues},
        HashMap,
    },
};
use futures::{stream::Stream, StreamExt};
use futures::SinkExt;
use tokio::sync::mpsc;
use std::sync::Arc;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time::{sleep, Duration, timeout};
const MESSAGES_PER_TICK: usize = 10; // 假设每次tick处理10个消息


/// The state for each connected client.
pub struct PeerHandler<C: Contribution + Unpin, N: NodeId + Unpin> {
    // Peer nid.
    nid: Option<N>,

    // The incoming stream of messages:
    wire_msgs: WireMessages<C, N>,

    /// Handle to the shared message state.
    hdb: Hydrabadger<C, N>,

    // TODO: Consider adding back a separate clone of `peer_internal_tx`. Is
    // there any difference if capacity isn't an issue? -- doubtful
    /// Receive half of the message channel.
    rx: WireRx<C, N>,

    /// Peer socket address.
    out_addr: OutAddr,
}

impl<C: Contribution + Unpin, N: NodeId + Unpin> PeerHandler<C, N> {
    /// Create a new instance of `Peer`.
    pub fn new(
        pub_info: Option<(N, InAddr, PublicKey)>,
        hdb: Hydrabadger<C, N>,
        mut wire_msgs: WireMessages<C, N>,
    ) -> PeerHandler<C, N> {
        // Get the client socket address
        let out_addr = OutAddr(wire_msgs.socket().peer_addr().unwrap());

        // Create a channel for this peer
        let (tx, rx) = mpsc::unbounded_channel();
        pub_info.as_ref().map(|(nid, _, _)| nid.clone());
        let nid = match pub_info {
            Some((ref nid, _, pk)) => {
                wire_msgs.set_peer_public_key(pk);
                Some(nid.clone())
            }
            None => None,
        };

        // Add an entry for this `Peer` in the shared state map.
        hdb.peers_mut().add(out_addr, tx, pub_info);

        PeerHandler {
            nid,
            wire_msgs,
            hdb,
            rx,
            out_addr,   
        }
    }

    pub(crate) fn hdb(&self) -> &Hydrabadger<C, N> {
        &self.hdb
    }

    pub(crate) fn out_addr(&self) -> &OutAddr {
        &self.out_addr
    }
}


//
impl<C: Contribution + Unpin, N: NodeId + Unpin> PeerHandler<C, N> {
    pub async fn handle(&mut self) -> Result<(), Error> {
        const MESSAGES_PER_TICK: usize = 10;

        loop {
            println!("..............peer内部开始进行处理...............");

            // NOTE: 备选方案
            for i in 0..MESSAGES_PER_TICK {
                // 收到从tx ---> rx管道接收的消息
                 match timeout(Duration::from_secs(2), self.rx.recv()).await {
                    Ok(Some(message)) => {
                        // info!("internal message is {:?}", i_msg);
                        // 然后将需要发出去的消息通过wire message进行TCP stream进行传送
                        info!("收到了wire message({:?})的消息", message.kind());
                        if let Err(e) = self.wire_msgs.start_send(message.clone()).await {
                            error!("Error while sending message: {}", e);
                        }
                        // info!("收到了wire message({:?})的消息，开始通过stream数据流发送........", message.kind());
                    }
                    Ok(None) => {
                        // The sending ends have all dropped.
                        info!("internal message channel has been closed...");
                        break;
                    }
                    Err(_) => {
                        // Handling time exceeded 2 seconds. yielding.
                        break;
                    }
                }
            }
            
            // TODO: 这里也是需要注意的一个点，有可能这里也会导致问题，目前还没研究清楚
            let _ = self.wire_msgs.flush().await?;
            println!("节点消息处理模块已结束， 开始接收wire message的stream流数据。。。。");
            
            // Read new messages from the socket
            // TODO: 目前的问题就在于，上面已经通过wire_msgs.send_msg发送出去了消息，但是这里却没有接收到wire msg！！！
            // NOTE: 注意，这里修改了，原来是while，现在改成了if
            if let Some(message) = self.wire_msgs.next().await {
                info!("Received message: {:?}", message);
                // handle message...
                match message {
                    Ok(msg) => {
                        match msg.into_kind() {
                            WireMessageKind::HelloRequestChangeAdd(src_nid, _in_addr, _pub_key) => {
                                error!(
                                    "Duplicate `WireMessage::HelloRequestChangeAdd` \
                                     received from '{:?}'",
                                    src_nid
                                );
                                info!("打个招呼,hello Request Change Add message");
                            }
                            WireMessageKind::WelcomeReceivedChangeAdd(src_nid, pk, net_state) => {
                                info!("打个招呼, welcome received Change Add message");
                                self.nid = Some(src_nid.clone());
                                self.wire_msgs.set_peer_public_key(pk);
                                self.hdb.send_internal(InternalMessage::wire(
                                    Some(src_nid.clone()),
                                    self.out_addr,
                                    WireMessage::welcome_received_change_add(
                                        src_nid.clone(),
                                        pk,
                                        net_state,
                                    ),
                                ));
                            }   
                            WireMessageKind::HelloFromValidator(src_nid, in_addr, pk, net_state) => {
                                self.nid = Some(src_nid.clone());
                                self.wire_msgs.set_peer_public_key(pk);
                                self.hdb.send_internal(InternalMessage::wire(
                                    Some(src_nid.clone()),
                                    self.out_addr,
                                    WireMessage::hello_from_validator(
                                        src_nid.clone(),
                                        in_addr,
                                        pk,
                                        net_state,
                                    ),
                                ));
                            }
                            WireMessageKind::Message(src_nid, msg) => {
                                if let Some(peer_nid) = self.nid.as_ref() {
                                    debug_assert_eq!(src_nid, *peer_nid);
                                }
        
                                self.hdb.send_internal(InternalMessage::hb_message(
                                    src_nid,
                                    self.out_addr,
                                    msg,
                                ))
                            }
                            WireMessageKind::Transaction(src_nid, txn) => {
                                if let Some(peer_nid) = self.nid.as_ref() {
                                    debug_assert_eq!(src_nid, *peer_nid);
                                }
        
                                self.hdb.send_internal(InternalMessage::hb_contribution(
                                    src_nid,
                                    self.out_addr,
                                    txn,
                                ))
                            }
                            kind => self.hdb.send_internal(InternalMessage::wire(
                                self.nid.clone(),
                                self.out_addr,
                                kind.into(),
                            )),
                        }
                    },
                    Err(e) => {
                        // handle error...
                        error!("Error when receiving message: {:?}", e);
                        return Err(e);
                    }
                }
            } else {
                info!("Peer ({}: '{:?}') disconnected.", self.out_addr, self.nid);
            }

            // 如果 next().await 返回 None，则表示 Stream 已经结束
            
            // Ok(());

            /*  else {
                info!("Peer ({}: '{:?}') disconnected.", self.out_addr, self.nid);
                // Remove peer transmitter from the lists:
                self.hdb.peers_mut().remove(&self.out_addr);
                if let Some(nid) = self.nid.clone() {
                    debug!(
                        "Sending peer ({}: '{:?}') disconnect internal message.",
                        self.out_addr,
                        self.nid.clone().unwrap()
                    );
        
                    self.hdb
                        .send_internal(InternalMessage::peer_disconnect(nid, self.out_addr));
                }
            } */
            // Ok(())
        }
    }
}


#[derive(Clone, Debug)]
#[allow(dead_code)]
enum State<N> {
    Handshaking,
    PendingJoinInfo {
        nid: N,
        in_addr: InAddr,
        pk: PublicKey,
    },
    EstablishedObserver {
        nid: N,
        in_addr: InAddr,
        pk: PublicKey,
    },
    EstablishedValidator {
        nid: N,
        in_addr: InAddr,
        pk: PublicKey,
    },
}

/// Nodes of the network.
#[derive(Clone, Debug)]
pub struct Peer<C: Contribution, N: NodeId> {
    out_addr: OutAddr,
    tx: WireTx<C, N>,
    state: State<N>,
}

impl<C: Contribution, N: NodeId> Peer<C, N> {
    /// Returns a new `Peer`
    fn new(
        out_addr: OutAddr,
        tx: WireTx<C, N>,
        pub_info: Option<(N, InAddr, PublicKey)>,
    ) -> Peer<C, N> {
        //FIXME: 这里是因为传入的pub_info没有值，所以这里并没有设置为EstablishedValidator，也是导致后面出现No in address的原因
        let state = match pub_info {
            None => State::Handshaking,
            Some((nid, in_addr, pk)) => State::EstablishedValidator { nid, in_addr, pk },
        };

        Peer {
            out_addr,
            tx,
            state,
        }
    }

    /// Sets a peer state to `State::PendingJoinInfo` and stores public info.
    fn set_pending(&mut self, pub_info: (N, InAddr, PublicKey)) {
        self.state = match self.state {
            State::Handshaking => State::PendingJoinInfo {
                nid: pub_info.0,
                in_addr: pub_info.1,
                pk: pub_info.2,
            },
            _ => panic!(
                "Peer::set_pending: Can only set pending when \
                 peer state is `Handshaking`."
            ),
        };
    }

    /// Sets a peer state to `State::EstablishedObserver` and stores public info.
    fn establish_observer(&mut self) {
        self.state = match self.state {
            State::PendingJoinInfo {
                ref nid,
                in_addr,
                pk,
            } => State::EstablishedObserver {
                nid: nid.clone(),
                in_addr,
                pk,
            },
            _ => panic!(
                "Peer::establish_observer: Can only establish observer when \
                 peer state is`PendingJoinInfo`."
            ),
        };
    }

    /// Sets a peer state to `State::EstablishedValidator` and stores public info.
    fn establish_validator(&mut self, pub_info: Option<(N, InAddr, PublicKey)>) {
        self.state = match self.state {
            State::Handshaking => match pub_info {
                Some(pi) => State::EstablishedValidator {
                    nid: pi.0,
                    in_addr: pi.1,
                    pk: pi.2,
                },
                None => {
                    panic!(
                        "Peer::establish_validator: `pub_info` must be supplied \
                         when establishing a validator from `Handshaking`."
                    );
                }
            },
            State::EstablishedObserver {
                ref nid,
                in_addr,
                pk,
            } => {
                if pub_info.is_some() {
                    panic!(
                        "Peer::establish_validator: `pub_info` must be `None` \
                         when upgrading an observer node."
                    );
                }
                State::EstablishedValidator {
                    nid: nid.clone(),
                    in_addr,
                    pk,
                }
            }
            _ => panic!(
                "Peer::establish_validator: Can only establish validator when \
                 peer state is`Handshaking` or `EstablishedObserver`."
            ),
        };
    }

    /// Returns the peer's unique identifier.
    pub fn node_id(&self) -> Option<&N> {
        match self.state {
            State::Handshaking => None,
            State::PendingJoinInfo { ref nid, .. } => Some(nid),
            State::EstablishedObserver { ref nid, .. } => Some(nid),
            State::EstablishedValidator { ref nid, .. } => Some(nid),
        }
    }

    /// Returns the peer's unique identifier.
    pub fn out_addr(&self) -> &OutAddr {
        &self.out_addr
    }

    /// Returns the peer's public key.
    pub fn public_key(&self) -> Option<&PublicKey> {
        match self.state {
            State::Handshaking => None,
            State::PendingJoinInfo { ref pk, .. } => Some(pk),
            State::EstablishedObserver { ref pk, .. } => Some(pk),
            State::EstablishedValidator { ref pk, .. } => Some(pk),
        }
    }

    /// Returns the peer's incoming (listening) socket address.
    pub fn in_addr(&self) -> Option<&InAddr> {
        match self.state {
            State::Handshaking => None,
            State::PendingJoinInfo { ref in_addr, .. } => Some(in_addr),
            State::EstablishedObserver { ref in_addr, .. } => Some(in_addr),
            State::EstablishedValidator { ref in_addr, .. } => Some(in_addr),
        }
    }

    /// Returns the peer's public info if established.
    pub fn pub_info(&self) -> Option<(&N, &InAddr, &PublicKey)> {
        match self.state {
            State::Handshaking => None,
            State::EstablishedObserver {
                ref nid,
                ref in_addr,
                ref pk,
            } => Some((nid, in_addr, pk)),
            State::PendingJoinInfo {
                ref nid,
                ref in_addr,
                ref pk,
            } => Some((nid, in_addr, pk)),
            State::EstablishedValidator {
                ref nid,
                ref in_addr,
                ref pk,
            } => Some((nid, in_addr, pk)),
        }
    }

    /// Returns true if this peer is pending.
    pub fn is_pending(&self) -> bool {
        match self.state {
            State::PendingJoinInfo { .. } => true,
            _ => false,
        }
    }

    /// Returns true if this peer is an established observer.
    pub fn is_observer(&self) -> bool {
        match self.state {
            State::EstablishedObserver { .. } => true,
            _ => false,
        }
    }

    /// Returns true if this peer is an established validator.
    pub fn is_validator(&self) -> bool {
        match self.state {
            State::EstablishedValidator { .. } => true,
            _ => false,
        }
    }

    /// Returns the peer's wire transmitter.
    pub fn tx(&self) -> &WireTx<C, N> {
        &self.tx
    }
}

/// Peer nodes of the network.
//
// TODO: Keep a separate `HashSet` of validator `OutAddrs` to avoid having to
// iterate through entire list.
#[derive(Debug)]
pub struct Peers<C: Contribution, N: NodeId> {
    // 记录本地节点连接的peer
    peers: HashMap<OutAddr, Peer<C, N>>,
    out_addrs: HashMap<N, OutAddr>,
    local_addr: InAddr,
}

impl<C: Contribution, N: NodeId> Peers<C, N> {
    /// Returns a new empty list of peers.
    pub(crate) fn new(local_addr: InAddr) -> Peers<C, N> {
        Peers {
            peers: HashMap::with_capacity(64),
            out_addrs: HashMap::with_capacity(64),
            local_addr,
        }
    }

    /// Adds a peer to the list.
    pub(crate) fn add(
        &mut self,
        out_addr: OutAddr,
        tx: WireTx<C, N>,
        pub_info: Option<(N, InAddr, PublicKey)>,
    ) {
        println!("即将添加到本节点的peer公钥信息为:{:?}", pub_info);
        let peer = Peer::new(out_addr, tx, pub_info);
        if let State::EstablishedValidator { ref nid, .. } = peer.state {
            self.out_addrs.insert(nid.clone(), peer.out_addr);
        }
        self.peers.insert(peer.out_addr, peer);
    }

    /// Attempts to set peer as pending-join-info, storing `pub_info`.
    ///
    /// Returns `true` if the peer was already pending.
    ///
    /// ### Panics
    ///
    /// Peer state must be `Handshaking`.
    ///
    /// TODO: Error handling...
    pub(crate) fn set_pending<O: Borrow<OutAddr>>(
        &mut self,
        out_addr: O,
        pub_info: (N, InAddr, PublicKey),
    ) -> bool {
        let peer = self.peers.get_mut(out_addr.borrow()).expect(&format!(
            "Peers::set_pending: \
             No peer found with outgoing address: {}",
            out_addr.borrow()
        ));
        match self
            .out_addrs
            .insert(pub_info.0.clone(), *out_addr.borrow())
        {
            Some(_out_addr_pub) => {
                let pi_pub = peer
                    .pub_info()
                    .expect("Peers::set_pending: internal consistency error");
                assert!(
                    pub_info.0 == *pi_pub.0 && pub_info.1 == *pi_pub.1 && pub_info.2 == *pi_pub.2
                );
                assert!(peer.is_validator());
                return true;
            }
            None => peer.set_pending(pub_info),
        }

        // false
        panic!("Peer::set_pending: Do not use yet.");
    }

    /// Attempts to establish a peer as an observer.
    ///
    /// ### Panics
    ///
    /// Peer state must be `Handshaking`.
    ///
    /// TODO: Error handling...
    pub(crate) fn establish_observer<O: Borrow<OutAddr>>(&mut self, out_addr: O) {
        let peer = self.peers.get_mut(out_addr.borrow()).expect(&format!(
            "Peers::establish_observer: \
             No peer found with outgoing address: {}",
            out_addr.borrow()
        ));

        // peer.establish_observer()
        panic!("Peer::set_pending: Do not use yet.");
    }

    /// Attempts to establish a peer as a validator, storing `pub_info`.
    ///
    /// Returns `true` if the peer was already an established validator.
    ///
    /// ### Panics
    ///
    /// Peer state must be `Handshaking` or `EstablishedObserver`.
    ///
    /// TODO: Error handling...
    pub(crate) fn establish_validator<O: Borrow<OutAddr>>(
        &mut self,
        out_addr: O,
        pub_info: (N, InAddr, PublicKey),
    ) -> bool {
        let borrowed_out_addr = out_addr.borrow();
        let peer = match self.peers.get_mut(borrowed_out_addr) {
            Some(peer) => peer,
            None => {
                eprintln!(
                    "Peers::establish_validator: \
                     No peer found with outgoing address: {}",
                    borrowed_out_addr
                );
                return false;
            }
        };
    
        let (node_id, in_addr, public_key) = pub_info.clone();
        if let Some(_) = self.out_addrs.insert(node_id.clone(), *borrowed_out_addr) {
            if let Some(pi_pub) = peer.pub_info() {
                assert!(
                    node_id == *pi_pub.0 && in_addr == *pi_pub.1 && public_key == *pi_pub.2,
                    "Peers::establish_validator: internal consistency error"
                );
                assert!(peer.is_validator());
                return true;
            } else {
                eprintln!("Peers::establish_validator: internal consistency error");
            }
        } else {
            peer.establish_validator(Some(pub_info));
        }
        false
    }

    pub(crate) fn wire_to_all(&self, msg: WireMessage<C, N>) {
        // NOTE: 给每个节点都发送
        for (_p_addr, peer) in self
            .peers
            .iter()
            .filter(|(&p_addr, _)| p_addr != OutAddr(self.local_addr.0))
        {

            // info!("当前要发送消息的validator节点是:{:?}", peer.clone());
            // NOTE: 可能是这里的问题，消息没有发出去！！！！
            // println!("当前要发送给validator的消息是{:?}", msg.clone());
            peer.tx().send(msg.clone()).unwrap();
        }
    }

    pub(crate) fn wire_to_validators(&self, msg: WireMessage<C, N>) {
        // for peer in peers.validators()   
        //         .filter(|p| p.out_addr() != &OutAddr(self.hdb.addr().0)) {
        //     peer.tx().unbounded_send(msg.clone()).unwrap();
        // }

        // FIXME: TEMPORARILY WIRE TO ALL FOR NOW.
        info!("开始wire to other validator!!!!!");
        self.wire_to_all(msg)
    }

    /// Sends a `WireMessage` to the target specified by `tar_nid`.
    ///
    /// If the target is not an established node, the message will be returned
    /// along with an incremented retry count.
    pub(crate) fn wire_to(
        &self,
        tar_nid: N,
        msg: WireMessage<C, N>,
        retry_count: usize,
    ) -> Option<(N, WireMessage<C, N>, usize)> {
        match self.get_by_nid(&tar_nid) {
            Some(p) => {
                p.tx().send(msg).unwrap();
                None
            }
            None => {
                info!(
                    "Node '{:?}' is not yet established. Queueing message for now (retry_count: {}).",
                    tar_nid, retry_count
                );
                Some((tar_nid, msg, retry_count + 1))
            }
        }
    }

    /// Removes a peer the list if it exists.
    pub(crate) fn remove<O: Borrow<OutAddr>>(&mut self, out_addr: O) {
        let peer = self.peers.remove(out_addr.borrow());
        if let Some(p) = peer {
            if let Some(nid) = p.node_id() {
                self.out_addrs.remove(&nid);
            }
        }
    }

    pub(crate) fn get<O: Borrow<OutAddr>>(&self, out_addr: O) -> Option<&Peer<C, N>> {
        self.peers.get(out_addr.borrow())
    }

    pub(crate) fn get_by_nid<U: Borrow<N>>(&self, nid: U) -> Option<&Peer<C, N>> {
        self.out_addrs
            .get(nid.borrow())
            .and_then(|addr| self.get(addr))
    }

    /// Returns an Iterator over the list of peers.
    pub(crate) fn iter(&self) -> HashMapIter<OutAddr, Peer<C, N>> {
        self.peers.iter()
    }

    /// Returns an Iterator over the list of peers.
    pub fn peers(&self) -> HashMapValues<OutAddr, Peer<C, N>> {
        self.peers.values()
    }

    /// Returns an iterator over the list of validators.
    pub fn validators(&self) -> impl Iterator<Item = &Peer<C, N>> {
        self.peers.values().filter(|p| p.is_validator())
    }

    /// Returns the current number of connected peers.
    pub fn count_total(&self) -> usize {
        self.peers.len()
    }

    /// Returns the current number of connected and established validators.
    ///
    /// This is semi-expensive (O(n)).
    pub fn count_validators(&self) -> usize {
        self.validators().count()
    }

    pub(crate) fn contains_in_addr<I: Borrow<InAddr>>(&self, in_addr: I) -> bool {
        for peer in self.peers.values() {
            if let Some(peer_in_addr) = peer.in_addr() {
                if peer_in_addr == in_addr.borrow() {
                    return true;
                }
            }
        }
        false
    }
}
