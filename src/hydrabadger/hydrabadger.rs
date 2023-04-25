//! A hydrabadger consensus node.
//!

use super::{Error, Handler, StateDsct, StateMachine};
use crate::peer::{PeerHandler, Peers};
use crate::{
    key_gen, BatchRx, Change, Contribution, EpochRx, EpochTx, InAddr, InternalMessage, InternalTx,
    NodeId, OutAddr, WireMessage, WireMessageKind, WireMessages,
};
use futures::{
    channel::mpsc,
    future::{self, Either},
};
use hbbft::crypto::{PublicKey, SecretKey};
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::de::DeserializeOwned;
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
/* use tokio::{
    self,
    net::{TcpListener, TcpStream},
    prelude::*,
    timer::{Delay, Interval},
}; */
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{interval, sleep};
use tokio_stream::{self as stream, StreamExt};

// The number of random transactions to generate per interval.
const DEFAULT_TXN_GEN_COUNT: usize = 5;
// The interval between randomly generated transactions.
const DEFAULT_TXN_GEN_INTERVAL: u64 = 5000;
// The number of bytes per randomly generated transaction.
const DEFAULT_TXN_GEN_BYTES: usize = 2;
// The minimum number of peers needed to spawn a HB instance.
const DEFAULT_KEYGEN_PEER_COUNT: usize = 2;
// Causes the primary hydrabadger thread to sleep after every batch. Used for
// debugging.
const DEFAULT_OUTPUT_EXTRA_DELAY_MS: u64 = 0;

/// Hydrabadger configuration options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub txn_gen_count: usize,
    pub txn_gen_interval: u64,
    pub txn_gen_bytes: usize,
    pub keygen_peer_count: usize,
    pub output_extra_delay_ms: u64,
    pub start_epoch: u64,
}

impl Config {
    pub fn with_defaults() -> Config {
        Config {
            txn_gen_count: DEFAULT_TXN_GEN_COUNT,
            txn_gen_interval: DEFAULT_TXN_GEN_INTERVAL,
            txn_gen_bytes: DEFAULT_TXN_GEN_BYTES,
            keygen_peer_count: DEFAULT_KEYGEN_PEER_COUNT,
            output_extra_delay_ms: DEFAULT_OUTPUT_EXTRA_DELAY_MS,
            start_epoch: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::with_defaults()
    }
}

struct Inner<C: Contribution, N: NodeId> {
    /// Node nid:
    nid: N,
    /// Incoming connection socket.
    addr: InAddr,

    /// This node's secret key.
    secret_key: SecretKey,

    peers: RwLock<Peers<C, N>>,

    /// The current state containing HB when connected.
    state: RwLock<StateMachine<C, N>>,

    /// A reference to the last known state discriminant. May be stale when read.
    state_dsct_stale: Arc<AtomicUsize>,

    // TODO: 节点内部的通道发送端
    peer_internal_tx: InternalTx<C, N>,

    /// The earliest epoch from which we have not yet received output.
    //
    // Used as an initial value when a new epoch listener is registered.
    current_epoch: Mutex<u64>,

    // TODO: Create a separate type which uses a hashmap internally and allows
    // for Tx removal. Alternatively just `Option` wrap Txs.
    epoch_listeners: RwLock<Vec<EpochTx>>,

    config: Config,
}

/// A `HoneyBadger` network node.
#[derive(Clone)]
pub struct Hydrabadger<C: Contribution, N: NodeId> {
    // 节点共享的一些参数
    inner: Arc<Inner<C, N>>,
    // 处理节点内部的消息
    handler: Arc<Mutex<Option<Handler<C, N>>>>,
    // 处理接收到的batch，也就是最终本地打包的交易
    batch_rx: Arc<Mutex<Option<BatchRx<C, N>>>>,
}

impl<C: Contribution, N: NodeId + DeserializeOwned + 'static> Hydrabadger<C, N> {
    /// Returns a new Hydrabadger node.
    pub fn new(addr: SocketAddr, cfg: Config, nid: N) -> Self {
        // 生成peer节点本地私钥
        let secret_key = SecretKey::random();

        // 创建两个通道(一个用来处理节点内部消息，即共识组件和其他组件之间的消息，一个用来发送和接收最终的batch)
        let (peer_internal_tx, peer_internal_rx) = mpsc::unbounded();
        let (batch_tx, batch_rx) = mpsc::unbounded();

        info!("");
        info!("Local HBBFT Node: ");
        info!("    UID:             {:?}", nid);
        info!("    Socket Address:  {}", addr);
        info!("    Public Key:      {:?}", secret_key.public_key());

        info!("");
        info!("****** Hello, You are starting a hbbft node! ******");
        info!("");

        // 记录epoch
        let current_epoch = cfg.start_epoch;

        // 初始化状态机
        let state = StateMachine::disconnected();
        let state_dsct_stale = state.dsct.clone();

        // 初始化Inner，包含了一个节点的必要信息
        let inner = Arc::new(Inner {
            nid,
            addr: InAddr(addr),
            secret_key,
            peers: RwLock::new(Peers::new(InAddr(addr))),
            state: RwLock::new(state),
            state_dsct_stale,
            peer_internal_tx,
            config: cfg,
            current_epoch: Mutex::new(current_epoch),
            epoch_listeners: RwLock::new(Vec::new()),
        });

        // 实例一个hdb节点
        let hdb = Hydrabadger {
            inner,
            handler: Arc::new(Mutex::new(None)),
            batch_rx: Arc::new(Mutex::new(Some(batch_rx))),
        };

        // 获得 handler的锁，新创建一个handler，将其传入
        *hdb.handler.lock() = Some(Handler::new(hdb.clone(), peer_internal_rx, batch_tx));

        // 返回实例
        hdb
    }

    /// Returns a new Hydrabadger node.
    pub fn with_defaults(addr: SocketAddr, nid: N) -> Self {
        Hydrabadger::new(addr, Config::default(), nid)
    }

    /// Returns the pre-created handler.
    pub fn handler(&self) -> Option<Handler<C, N>> {
        self.handler.lock().take()
    }

    /// Returns the batch output receiver.
    pub fn batch_rx(&self) -> Option<BatchRx<C, N>> {
        self.batch_rx.lock().take()
    }

    /// Returns a reference to the inner state.
    pub fn state(&self) -> RwLockReadGuard<StateMachine<C, N>> {
        self.inner.state.read()
    }

    /// Returns a mutable reference to the inner state.
    pub(crate) fn state_mut(&self) -> RwLockWriteGuard<StateMachine<C, N>> {
        self.inner.state.write()
    }

    /// Returns a recent state discriminant.
    ///
    /// The returned value may not be up to date and must be considered
    /// immediately stale.
    pub fn state_dsct_stale(&self) -> StateDsct {
        self.inner.state_dsct_stale.load(Ordering::Relaxed).into()
    }

    pub fn is_validator(&self) -> bool {
        self.state_dsct_stale() == StateDsct::Validator
    }

    /// Returns a reference to the peers list.
    pub fn peers(&self) -> RwLockReadGuard<Peers<C, N>> {
        self.inner.peers.read()
    }

    /// Returns a mutable reference to the peers list.
    pub(crate) fn peers_mut(&self) -> RwLockWriteGuard<Peers<C, N>> {
        self.inner.peers.write()
    }

    /// Sets the current epoch and returns the previous epoch.
    ///
    /// The returned value should (always?) be equal to `epoch - 1`.
    //
    // TODO: Convert to a simple incrementer?
    pub(crate) fn set_current_epoch(&self, epoch: u64) -> u64 {
        let mut ce = self.inner.current_epoch.lock();
        let prev_epoch = *ce;
        *ce = epoch;
        prev_epoch
    }

    /// Returns the epoch of the next batch to be output.
    pub fn current_epoch(&self) -> u64 {
        *self.inner.current_epoch.lock()
    }

    /// Returns a stream of epoch numbers (e) indicating that a batch has been
    /// output for an epoch (e - 1) and that a new epoch has begun.
    ///
    /// The current epoch will be sent upon registration. If a listener is
    /// registered before any batches have been output by this instance of
    /// Hydrabadger, the start epoch will be output.
    pub fn register_epoch_listener(&self) -> EpochRx {
        let (tx, rx) = mpsc::unbounded();
        if self.is_validator() {
            tx.unbounded_send(self.current_epoch())
                .expect("Unknown error: receiver can not have dropped");
        }
        self.inner.epoch_listeners.write().push(tx);
        rx
    }

    /// Returns a reference to the epoch listeners list.
    pub(crate) fn epoch_listeners(&self) -> RwLockReadGuard<Vec<EpochTx>> {
        self.inner.epoch_listeners.read()
    }

    /// Returns a reference to the config.
    pub(crate) fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Sends a message on the internal tx.
    pub(crate) fn send_internal(&self, msg: InternalMessage<C, N>) {
        if let Err(err) = self.inner.peer_internal_tx.unbounded_send(msg) {
            error!(
                "Unable to send on internal tx. Internal rx has dropped: {}",
                err
            );
            ::std::process::exit(-1)
        }
    }

    /// Handles a incoming batch of user transactions.
    pub fn propose_user_contribution(&self, txn: C) -> Result<(), Error> {
        if self.is_validator() {
            self.send_internal(InternalMessage::hb_contribution(
                self.inner.nid.clone(),
                OutAddr(*self.inner.addr),
                txn,
            ));
            Ok(())
        } else {
            Err(Error::ProposeUserContributionNotValidator)
        }
    }

    /// Casts a vote for a change in the validator set or configuration.
    pub fn vote_for(&self, change: Change<N>) -> Result<(), Error> {
        if self.is_validator() {
            self.send_internal(InternalMessage::hb_vote(
                self.inner.nid.clone(),
                OutAddr(*self.inner.addr),
                change,
            ));
            Ok(())
        } else {
            Err(Error::VoteForNotValidator)
        }
    }

    /// Begins a synchronous distributed key generation instance and returns a
    /// stream which may be polled for events and messages.
    pub fn new_key_gen_instance(&self) -> mpsc::UnboundedReceiver<key_gen::Message> {
        let (tx, rx) = mpsc::unbounded();
        self.send_internal(InternalMessage::new_key_gen_instance(
            self.inner.nid.clone(),
            OutAddr(*self.inner.addr),
            tx,
        ));
        rx
    }

    /// Returns a future that handles incoming connections on `socket`.
    async fn handle_incoming(self, socket: TcpStream) {
        info!("Incoming connection from '{}'", socket.peer_addr().unwrap());
        let wire_msgs: WireMessages<C, N> =
            WireMessages::new(socket, self.inner.secret_key.clone());

        let result = async move {
            let mutwire_msgs_pin = Box::pin(wire_msgs);
            let msg_opt = wire_msgs_pin.next().await;
            // 判断是不是有message传到本地节点
            match msg_opt {
                // 如果有，根据wiremessage的类型，做对应处理
                Some(msg) => match msg.expect("Connection error").into_kind() {
                    // 节点连接请求的消息
                    WireMessageKind::HelloRequestChangeAdd(peer_nid, peer_in_addr, peer_pk) => {
                        // NOTE: 调用PeerHandler::new方法处理，处理节点外部来的消息
                        let peer_h = PeerHandler::new(
                            Some((peer_nid.clone(), peer_in_addr, peer_pk)),
                            self.clone(),
                            wire_msgs,
                        );
                        // NOTE: 将HelloRequestChangeAdd消息在节点内部进一步处理hdb().send_internal
                        peer_h
                            .hdb()
                            .send_internal(InternalMessage::new_incoming_connection(
                                peer_nid.clone(),
                                *peer_h.out_addr(),
                                peer_in_addr,
                                peer_pk,
                                true,
                            ));
                        Ok(peer_h)
                    }
                    _ => {
                        error!(
                            "Peer connected without sending \
                                 `WireMessageKind::HelloRequestChangeAdd`."
                        );
                        Err(())
                    }
                },
                None => Err(()),
            }
        }
        .await;

        match result {
            Ok(peer_h) => {
                // Handle the peer here, e.g.:
                // peer_h.process().await;
            }
            Err(_) => {
                error!("Connection error");
            }
        }
    }

    /// Returns a future that connects to new peer.
    pub(super) async fn connect_outgoing(
        self,
        remote_addr: SocketAddr,
        local_sk: SecretKey,
        pub_info: Option<(N, InAddr, PublicKey)>,
        is_optimistic: bool,
    ) {
        let nid = self.inner.nid.clone();
        let in_addr = self.inner.addr;

        info!("Initiating outgoing connection to: {}", remote_addr);

        // NOTE: 和远程节点建立TCP连接
        let result = TcpStream::connect(remote_addr).await;

        match result {
            Ok(socket) => {
                let local_pk = local_sk.public_key();
                // Wrap the socket with the frame delimiter and codec:
                let mut wire_msgs = WireMessages::new(socket, local_sk);
                // NOTE: 构建一个wireMessage，类型为hello_request_change_add，准备请求加入，然后发送出去
                let wire_hello_result = wire_msgs
                    .send_msg(WireMessage::hello_request_change_add(
                        nid, in_addr, local_pk,
                    ))
                    .await;

                match wire_hello_result {
                    Ok(_) => {
                        let peer = PeerHandler::new(pub_info, self.clone(), wire_msgs);

                        self.send_internal(InternalMessage::new_outgoing_connection(
                            *peer.out_addr(),
                        ));
                    }
                    Err(err) => {
                        if is_optimistic {
                            warn!(
                                "Unable to connect to: {} ({e:?}: {e})",
                                remote_addr,
                                e = err
                            );
                        } else {
                            error!("Error connecting to: {} ({e:?}: {e})", remote_addr, e = err);
                        }
                    }
                }
            }
            Err(err) => {
                if is_optimistic {
                    warn!(
                        "Unable to connect to: {} ({e:?}: {e})",
                        remote_addr,
                        e = err
                    );
                } else {
                    error!("Error connecting to: {} ({e:?}: {e})", remote_addr, e = err);
                }
            }
        }
    }

    // 产生contribution交易
    async fn generate_contributions(
        self,
        // 注意这里传入的时候是一个闭包
        gen_txns: Option<fn(usize, usize) -> C>,
    ) {
        if let Some(gen_txns) = gen_txns {
            let epoch_stream = self.register_epoch_listener();
            let gen_delay = self.inner.config.txn_gen_interval;
            // 每隔一个时间间隔生成一个contribution
            let gen_cntrb = async move {
                loop {
                    let epoch_no = epoch_stream.next().await;
                    match epoch_no {
                        Some(_) => {
                            tokio::time::sleep(Duration::from_millis(gen_delay)).await;

                            let hdb = self.clone();

                            if let StateDsct::Validator = hdb.state_dsct_stale() {
                                info!(
                                    "Generating and sending {} random transactions...",
                                    self.inner.config.txn_gen_count
                                );

                                let txns = gen_txns(
                                    self.inner.config.txn_gen_count,
                                    self.inner.config.txn_gen_bytes,
                                );

                                println!("我们本地的节点产生的随机交易数据为{:?}", txns);

                                hdb.send_internal(InternalMessage::hb_contribution(
                                    hdb.inner.nid.clone(),
                                    OutAddr(*hdb.inner.addr),
                                    txns,
                                ));
                            }
                        }
                        None => break,
                    }
                }
            };
            // 运行 generate_contributions
            gen_cntrb
                .await
                .map_err(|err| panic!("Contribution generation error: {:?}", err));
        };
    }

    /// Returns a future that generates random transactions and logs status
    /// messages.
    async fn log_status(self) {
        let log_interval = Duration::from_millis(self.inner.config.txn_gen_interval);
        let mut interval = interval(log_interval);

        while let Some(_) = interval.next().await {
            let hdb = self.clone();
            let peers = hdb.peers();

            // Log state:
            let dsct = hdb.state_dsct_stale();
            let peer_count = peers.count_total();
            info!(
                "Current Node Role State: {:?}(connect with {} peer nodes)",
                dsct, peer_count
            );

            // Log peer list:
            let peer_list = peers
                .peers()
                .map(|p| {
                    p.in_addr()
                        .map(|ia| ia.0.to_string())
                        .unwrap_or(format!("No in address"))
                })
                .collect::<Vec<_>>();
            info!("    Peers: {:?}", peer_list);

            // Log (trace) full peerhandler details:
            trace!("PeerHandler list:");
            for (peer_addr, _peer) in peers.iter() {
                trace!("     peer_addr: {}", peer_addr);
            }

            drop(peers);
        }
    }

    /// Binds to a host address and returns a future which starts the node.
    pub async fn node(
        self,
        remotes: Option<HashSet<SocketAddr>>,
        gen_txns: Option<fn(usize, usize) -> C>,
    ) {
        let socket = TcpListener::bind(&self.inner.addr).unwrap();
        info!("Listening on: {}", self.inner.addr);

        let remotes = remotes.unwrap_or_default();

        let hdb = self.clone();
        let local_sk = hdb.inner.secret_key.clone();

        // 1. 与远程节点建立连接（从节点角度来看，是消息出去）
        for &remote_addr in remotes.iter().filter(|&&ra| ra != hdb.inner.addr.0) {
            let hdb_clone = hdb.clone();
            let local_sk_clone = local_sk.clone();
            tokio::spawn(async move {
                hdb_clone
                    .connect_outgoing(remote_addr, local_sk_clone, None, true)
                    .await;
            });
        }

        // 0. 与远程节点建立连接(从节点角度看，是消息进来)
        let hdb_clone = self.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = socket.accept().await {
                let hdb_inner_clone = hdb_clone.clone();
                tokio::spawn(async move {
                    hdb_inner_clone.handle_incoming(socket).await;
                });
            }
        });

        // 2. Hydrabadger handler
        tokio::spawn(async move {
            self.handler().await.unwrap_or_else(|err| {
                error!("Handler internal error: {:?}", err);
            });
        });

        // 3. Log status
        let hdb_clone = self.clone();
        tokio::spawn(async move {
            hdb_clone.log_status().await;
        });

        // 4. Generate contributions
        let hdb_clone = self.clone();
        tokio::spawn(async move {
            hdb_clone.generate_contributions(gen_txns).await;
        });

        // 5. 起一个单独的协程，专门来消费batch数据，打包为区块
        let hdb_clone = self.clone();
        tokio::spawn(async move {
            let mut batch_receiver = hdb_clone.batch_rx().unwrap();

            while let Some(block) = batch_receiver.recv().await {
                println!("本地得到的HBBFT共识结果为{:?}", block);
            }
        });
    }

    /// Starts a node.
    pub async fn run_node(
        self,
        remotes: Option<HashSet<SocketAddr>>,
        gen_txns: Option<fn(usize, usize) -> C>,
    ) {
        self.node(remotes, gen_txns).await;
    }

    pub fn addr(&self) -> &InAddr {
        &self.inner.addr
    }

    pub fn node_id(&self) -> &N {
        &self.inner.nid
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.inner.secret_key
    }
}
