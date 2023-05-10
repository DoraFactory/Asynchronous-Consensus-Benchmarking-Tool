//! A hydrabadger consensus node.
//!

use super::{Error, Handler, StateDsct, StateMachine};
use crate::peer::{PeerHandler, Peers};
use crate::{
    key_gen, BatchRx, Change, Contribution, EpochRx, EpochTx, InAddr, InternalMessage, InternalTx,
    NodeId, OutAddr, WireMessage, WireMessageKind, WireMessages, Transaction,
};
use futures::{
    future::{self, Either},
    sync::mpsc,
};
use hbbft::crypto::{PublicKey, SecretKey};
use hbbft::dynamic_honey_badger::Batch;
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
// 测试数据的写入
use std::fs::{File, OpenOptions, metadata};
use std::io::{Write, BufWriter};
use tokio::{
    self,
    net::{TcpListener, TcpStream},
    prelude::*,
    timer::{Delay, Interval},
};

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

impl<C: Contribution, N: NodeId + DeserializeOwned + 'static > Hydrabadger<C, N> {
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

    /// Return peer node id
    pub fn get_nid(&self) -> String {
        self.inner.nid.to_string()
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
    fn handle_incoming(self, socket: TcpStream) -> impl Future<Item = (), Error = ()> {
        info!("Incoming connection from '{}'", socket.peer_addr().unwrap());
        let wire_msgs: WireMessages<C, N> =
            WireMessages::new(socket, self.inner.secret_key.clone());

        wire_msgs
            .into_future()
            .map_err(|(e, _)| e)
            .and_then(move |(msg_opt, w_messages)| {

                // 判断是不是有message传到本地节点
                match msg_opt {
                    // 如果有，根据wiremessage的类型，做对应处理
                    Some(msg) => match msg.into_kind() {
                        // 节点连接请求
                        WireMessageKind::HelloRequestChangeAdd(peer_nid, peer_in_addr, peer_pk) => {
                            // 将节点添加到本地节点的列表
                            // NOTE: 调用PeerHandler::new方法处理，处理节点外部来的消息
                            let peer_h = PeerHandler::new(
                                Some((peer_nid.clone(), peer_in_addr, peer_pk)),
                                self.clone(),
                                w_messages,
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
                            Either::B(peer_h)
                        }
                        _ => {
                            // TODO: Return this as a future-error (handled below):
                            error!(
                                "Peer connected without sending \
                                 `WireMessageKind::HelloRequestChangeAdd`."
                            );
                            Either::A(future::ok(()))
                        }
                    },
                    None => {
                        // The remote client closed the connection without sending
                        // a welcome_request_change_add message.
                        Either::A(future::ok(()))
                    }
                }
            })
            .map_err(|err| error!("Connection error = {:?}", err))
    }

    /// Returns a future that connects to new peer.
    pub(super) fn connect_outgoing(
        self,
        remote_addr: SocketAddr,
        local_sk: SecretKey,
        pub_info: Option<(N, InAddr, PublicKey)>,
        is_optimistic: bool,
    ) -> impl Future<Item = (), Error = ()> {
        let nid = self.inner.nid.clone();
        let in_addr = self.inner.addr;

        info!("Initiating outgoing connection to: {}", remote_addr);

        // NOTE: 和远程节点建立TCP连接
        TcpStream::connect(&remote_addr)
            .map_err(Error::from)
            .and_then(move |socket| {
                let local_pk = local_sk.public_key();
                // Wrap the socket with the frame delimiter and codec:
                let mut wire_msgs = WireMessages::new(socket, local_sk);
                // NOTE: 构建一个wireMessage，类型为hello_request_change_add，准备请求加入，然后发送出去
                let wire_hello_result = wire_msgs.send_msg(WireMessage::hello_request_change_add(
                    nid, in_addr, local_pk,
                ));
                // 获取请求连接的结果，如果这个连接成功，就在当前节点内部发送new_outgoing_connection消息
                match wire_hello_result {
                    Ok(_) => {
                        let peer = PeerHandler::new(pub_info, self.clone(), wire_msgs);

                        self.send_internal(InternalMessage::new_outgoing_connection(
                            *peer.out_addr(),
                        ));

                        Either::A(peer)
                    }
                    Err(err) => Either::B(future::err(err)),
                }
            })
            .map_err(move |err| {
                if is_optimistic {
                    warn!(
                        "Unable to connect to: {} ({e:?}: {e})",
                        remote_addr,
                        e = err
                    );
                } else {
                    error!("Error connecting to: {} ({e:?}: {e})", remote_addr, e = err);
                }
            })
    }

    // 产生contribution交易
    fn generate_contributions(
        self,
        // 注意这里传入的时候是一个闭包
        gen_txns: Option<fn(usize, usize) -> C>,
    ) -> impl Future<Item = (), Error = ()> {
        if let Some(gen_txns) = gen_txns {
            let epoch_stream = self.register_epoch_listener();
            let gen_delay = self.inner.config.txn_gen_interval;
            // 每隔一个时间间隔生成一个contribution
            let gen_cntrb = epoch_stream
                .and_then(move |epoch_no| {
                    Delay::new(Instant::now() + Duration::from_millis(gen_delay))
                        .map_err(|err| panic!("Timer error: {:?}", err))
                        .and_then(move |_| Ok(epoch_no))
                })
                .for_each(move |_epoch_no| {
                    let hdb = self.clone();

                    if let StateDsct::Validator = hdb.state_dsct_stale() {
                        info!(
                            "Generating and sending {} random transactions...",
                            self.inner.config.txn_gen_count
                        );
                        // Send some random transactions to our internal HB instance.
                        // 这里会实际调用匿名函数，产生随机交易
                        let txns = gen_txns(
                            self.inner.config.txn_gen_count,
                            self.inner.config.txn_gen_bytes,
                        );

                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        println!("我们本地的节点产生的随机交易数据为{:?}", txns);
                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");

                        // 发送节点内部消息
                        hdb.send_internal(InternalMessage::hb_contribution(
                            hdb.inner.nid.clone(),
                            OutAddr(*hdb.inner.addr),
                            // contribution是一个泛型，可以是任何类型的数据，比如具体的交易等等
                            txns,
                        ));
                    }
                    Ok(())
                })
                .map_err(|err| panic!("Contribution generation error: {:?}", err));

            Either::A(gen_cntrb)
        } else {
            Either::B(future::ok(()))
        }
    }

    /// Returns a future that generates random transactions and logs status
    /// messages.
    fn log_status(self) -> impl Future<Item = (), Error = ()> {
        Interval::new(
            Instant::now(),
            Duration::from_millis(self.inner.config.txn_gen_interval),
        )
        .for_each(move |_| {
            let hdb = self.clone();
            let peers = hdb.peers();

            // Log state:
            let dsct = hdb.state_dsct_stale();
            let peer_count = peers.count_total();
            info!("Current Node Role State: {:?}(connect with {} peer nodes)", dsct, peer_count);

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

            Ok(())
        })
        .map_err(|err| panic!("List connection interval error: {:?}", err))
    }

    /// Binds to a host address and returns a future which starts the node.
    pub fn node(
        self,
        remotes: Option<HashSet<SocketAddr>>,
        gen_txns: Option<fn(usize, usize) -> C>,
    ) -> impl Future<Item = (), Error = ()> {
        let socket = TcpListener::bind(&self.inner.addr).unwrap();
        info!("Listening on: {}", self.inner.addr);

        let remotes = remotes.unwrap_or_default();

        let hdb = self.clone();
        // 0. 与远程节点建立连接(从节点角度看，是消息进来)
        let listen = socket
            .incoming()
            .map_err(|err| error!("Error accepting socket: {:?}", err))
            // 对于每个过来的connection，都会新建一个协程与其连接通信，同时调用hdb的handle_incoming方法来处理即将收到的消息
            .for_each(move |socket| {
                // NOTE: hdb.clone().handle_incoming   (主要是TCP连接)
                tokio::spawn(hdb.clone().handle_incoming(socket));
                Ok(())
            });

        let hdb = self.clone();
        // 1. 与远程节点建立连接（从节点角度来看，是消息出去）
        // NOTE: 这里的处理流程是： 其他节点connect本地节点，本地节点通过listen监听到消息
        let local_sk = hdb.inner.secret_key.clone();
        let connect = future::lazy(move || {
            for &remote_addr in remotes.iter().filter(|&&ra| ra != hdb.inner.addr.0) {
                // NOTE: hdb.clone().connect_outgoing
                tokio::spawn(hdb.clone().connect_outgoing(
                    remote_addr,
                    local_sk.clone(),
                    None,
                    true,
                ));
            }
            Ok(())
        });

        // 2 NOTE: hydrabadger.handle()
        let hdb_handler = self
            .handler()
            .map_err(|err| error!("Handler internal error: {:?}", err));

        // 3 记录log
        let log_status = self.clone().log_status();

        // 4. 产生contribution，这个后续可以通过queueing hdb，目前只是dhb
        let generate_contributions = self.clone().generate_contributions(gen_txns);

        // 5. 起一个单独的协程，专门来消费batch数据，打包为区块
        let hdb_clone = self.clone();
        let produce_block = future::lazy(move || {
            let batch_receiver = hdb_clone.batch_rx().unwrap();
            let mut last_block_time = Instant::now();
            batch_receiver.for_each(move |block| {
                let current_epoch = block.epoch();
                println!("********************current epoch:{:?}***********************************", current_epoch);
                // Calculate the time interval since the last block
                let interval = Instant::now().duration_since(last_block_time);
                println!("**********************************epoch interval time:{:?}*****************************", interval);
                last_block_time = Instant::now(); // Update the last_block_time
                println!("*****");
                let contributor_list: Vec<_> = block.contributions().map(|(validator, _)| validator).collect();
                let block_txs: Vec<_> = block.contributions().map(|(_, value)| value).collect();
                println!("***********************************contributor list is {:?}*************************************", contributor_list);
                println!("************************************output {:?} transactions*****************************************", block_txs.len() * self.inner.config.txn_gen_count);
                println!("****************************************************************");

                // validator数量
                let validator_numbers = hdb_clone.peers().count_validators() + 1;

                // 将测试结果以nodeid命名的md文件
                let file_name = hdb_clone.get_nid() + ".md";
                let metadata = metadata(&file_name);
                let should_write_headers = match metadata {
                    Ok(meta) => meta.len() == 0,
                    Err(_) => true,
                };

                // 打开文件，如果不存在，则创建一个新文件
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open(&file_name)
                    .unwrap();

                let mut writer = BufWriter::new(file);

                if should_write_headers {
                    let headers = ["epoch_id", "validator num", "contributor num", "batch num", "tx size(Bytes)/tx", "block num", "epoch_time(latency)"];
                    let header_line = format!("\n | {} | {} | {} | {} | {} | {} | {} |\n", headers[0], headers[1], headers[2], headers[3], headers[4], headers[5], headers[6]);
                    let separator_line = "|:-------:|:-------:|:-------:|:-------:|:-------:|:-------:|:-------:|\n";
                    writer.write_all(header_line.as_bytes()).unwrap();
                    writer.write_all(separator_line.as_bytes()).unwrap();
                }
                
                let epoch_time = interval.as_secs_f64();
                // prepare data
                let data = vec![
                    current_epoch.to_string(), 
                    validator_numbers.to_string(), 
                    contributor_list.len().to_string(), 
                    self.inner.config.txn_gen_count.to_string(), 
                    self.inner.config.txn_gen_bytes.to_string(), (block_txs.len() * self.inner.config.txn_gen_count).to_string(), 
                    interval.as_secs_f64().to_string()
                ];

                // write data
                let data_format = format!("| {} | {} | {} | {} | {} | {} | {} |\n", data[0], data[1], data[2], data[3], data[4], data[5], data[6]);
                writer.write_all(data_format.as_bytes()).unwrap();
                Ok(())
            })
        });

        // 监听所有的协程
        listen
            .join5(connect, hdb_handler, log_status, generate_contributions)
            .map(|(..)| ())
            .join(produce_block)
            .map(|(..)| ())

    }

    /// Starts a node.
    pub fn run_node(
        self,
        remotes: Option<HashSet<SocketAddr>>,
        gen_txns: Option<fn(usize, usize) -> C>,
    ) {
        tokio::run(self.node(remotes, gen_txns));
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

    pub fn sub_block_tx_count(&self) -> usize {
        self.inner.config.txn_gen_count
    }
}