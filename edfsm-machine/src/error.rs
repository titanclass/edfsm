use derive_more::From;

/// Result type for this module
pub type Result<A> = core::result::Result<A, Error>;

/// Error type for this module
#[derive(Debug, Clone, From)]
pub enum Error {
    ChannelClosed,
}

#[cfg(feature = "tokio")]
pub mod adapt_tokio {
    use super::Error;
    use tokio::sync::{broadcast, mpsc, oneshot};

    impl<E> From<mpsc::error::SendError<E>> for Error {
        fn from(_: mpsc::error::SendError<E>) -> Self {
            Error::ChannelClosed
        }
    }

    impl<E> From<broadcast::error::SendError<E>> for Error {
        fn from(_: broadcast::error::SendError<E>) -> Self {
            Error::ChannelClosed
        }
    }

    impl From<oneshot::error::RecvError> for Error {
        fn from(_: oneshot::error::RecvError) -> Self {
            Error::ChannelClosed
        }
    }
}
