use thiserror::Error;
use uds_rw::{UdsError, UdsMessage};

#[derive(Error, Debug)]
pub enum ScenarioError {
    #[error("Network connector dead")]
    NetworkConnectorDead,
    #[error("DoIp routing activation failed")]
    RoutingActivationFailed,
    #[error(transparent)]
    UdsError(#[from] UdsError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("NRC received and not handled : {0}")]
    Nrc(u8),
    #[error("Unexpected UDS message received: {0:?}")]
    UnexpectedUdsMessage(UdsMessage),
}
