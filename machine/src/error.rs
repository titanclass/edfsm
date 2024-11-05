use derive_more::From;

/// Result type for this module
pub type Result<A> = std::result::Result<A, Error>;

/// Error type for this module
#[derive(Debug, Clone, From)]
pub enum Error {
    ChannelClosed,
}

#[cfg(feature = "tokio")]
pub mod adapt_channel {
    use super::Error;
    use tokio::sync::{broadcast, mpsc};

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
}
