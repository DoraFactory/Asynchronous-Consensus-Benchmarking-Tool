mod handler;
mod hydrabadger;
pub mod key_gen;
mod state;

use self::handler::Handler;
use self::state::{State, StateMachine};
use crate::{Change, Message};
use bincode;
use hbbft::{dynamic_honey_badger::Error as DhbError, sync_key_gen::Error as SyncKeyGenError};
use std;

pub use self::hydrabadger::{Config, Hydrabadger};
pub use self::state::StateDsct;
use std::error::Error as StdError;
use std::fmt;

// Number of times to attempt wire message re-send.
pub const WIRE_MESSAGE_RETRY_MAX: usize = 10;

/// A HoneyBadger input or message.
#[derive(Clone, Debug)]
pub enum InputOrMessage<T, N: Ord> {
    Change(Change<N>),
    Contribution(T),
    Message(N, Message<N>),
}

// TODO: Move this up to `lib.rs` or, preferably, create another error type
// for general (lib) use.
#[derive(Debug)]
pub enum Error {
    // #[fail(display = "Io error: {}", _0)] 
    Io(std::io::Error),
    // #[fail(display = "Serde error: {}", _0)]
    Serde(bincode::Error),
    // #[fail(display = "Connecting error")]
    ConnectError,
    // #[fail(display = "Handler internal error: Handler is None")]
    HandlerInternalError,
    // #[fail(display = "Received a message with invalid signature")]
    InvalidSignature,
    // #[fail(display = "Error polling hydrabadger internal receiver")]
    HydrabadgerHandlerPoll,
    // #[fail(display = "DynamicHoneyBadger error")]
    Dhb(DhbError),
    // #[fail(display = "DynamicHoneyBadger step error")]
    HbStep(DhbError),
    // #[fail(display = "Error creating SyncKeyGen: {}", _0)]
    SyncKeyGenNew(SyncKeyGenError),
    // #[fail(display = "Error generating keys: {}", _0)]
    SyncKeyGenGenerate(SyncKeyGenError),
    // #[fail(display = "Unable to push user transactions, this node is not a validator")]
    ProposeUserContributionNotValidator,
    // #[fail(display = "Unable to vote for a change, this node is not a validator")]
    VoteForNotValidator,
    // #[fail(display = "Unable to transmit epoch status to listener, listener receiver dropped")]
    InstantiateHbListenerDropped,
    // #[fail(display = "Message received from unknown peer while attempting to verify")]
    VerificationMessageReceivedUnknownPeer,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Error {
        Error::Serde(err)
    }
}

impl From<DhbError> for Error {
    fn from(err: DhbError) -> Error {
        Error::Dhb(err)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Serde(err) => Some(err),
            // Error::Dhb(err) => Some(err),
/*             Error::ConnectError(err) => Some(err),
            Error::HandlerInternalError(err) =? 
            Error::InvalidSignature
            Error::HydrabadgerHandlerPoll
            Error::HbStep
            Error::SyncKeyGenNew
            Error::SyncKeyGenGenerate
            Error::ProposeUserContributionNotValidator
            Error::VoteForNotValidator
            Error::InstantiateHbListenerDropped
            error::VerificationMessageReceivedUnknownPeer */
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 格式化错误信息的实现
        write!(f, "Custom Error: {}", self)
    }
}