use std::{io, io::Cursor, net::SocketAddr, time::Duration};

use super::error::ScenarioError;
use doip_rw::{message::UdsBuffer, LogicalAddress};

use uds_rw::{uds_write, UdsMessage};

use doip_rw_tokio::{DoIpCnxError, DoIpTcpConnection, Timings};

#[derive(Debug)]
pub enum ScenarioMessage {
    Uds(UdsMessage),
    AliveCheckReq,
    AliveCheckRsp,
    DisconnectReconnectReq,
    NotifyNewDoIpCnx,
    NotifyDoIpCnxRoutingAck,
}

pub struct DoIpConnection {
    connection: Option<DoIpTcpConnection>,
    la: LogicalAddress,
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
    timings: Timings,
    notify_new_cnx: bool,
    notify_doip_routed: bool,
    ack_buffer_holder: Option<Vec<u8>>,
    receive_buffer_holder: Option<Vec<u8>>,
}

impl DoIpConnection {
    pub async fn connect(
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        la: LogicalAddress,
    ) -> Result<DoIpConnection, ScenarioError> {
        let timings = Timings {
            tcp_connect: Duration::from_secs(1),
            routing_activation_rsp: Duration::from_secs(1),
        };
        let connection =
            DoIpTcpConnection::connect_doip_tcp(local_addr, remote_addr, la, timings.clone())
                .await?;
        Ok(DoIpConnection {
            connection: Some(connection),
            la,
            local_addr,
            remote_addr,
            timings,
            notify_new_cnx: false,
            notify_doip_routed: false,
            ack_buffer_holder: Some(vec![]),
            receive_buffer_holder: Some(vec![]),
        })
    }

    pub async fn send_scenario(
        &mut self,
        ta: LogicalAddress,
        scenario_req: ScenarioMessage,
    ) -> Result<(), ScenarioError> {
        use ScenarioMessage::*;
        match scenario_req {
            Uds(uds) => send_uds(self, ta, uds).await,
            AliveCheckReq => Ok(()),
            AliveCheckRsp => send_alive_check_rsp(self).await,
            DisconnectReconnectReq => self.reconnect().await,
            NotifyNewDoIpCnx => Ok(()),
            NotifyDoIpCnxRoutingAck => Ok(()),
        }
    }

    pub async fn recv_scenario(&mut self) -> Result<ScenarioMessage, ScenarioError> {
        if self.notify_new_cnx {
            self.notify_new_cnx = false;
            Ok(ScenarioMessage::NotifyNewDoIpCnx)
        } else if self.notify_doip_routed {
            self.notify_doip_routed = false;
            Ok(ScenarioMessage::NotifyDoIpCnxRoutingAck)
        } else {
            let scenario_msg = doip_scenario_receive(self).await?;
            Ok(scenario_msg)
        }
    }

    pub async fn reconnect(&mut self) -> Result<(), ScenarioError> {
        if let Some(old) = self.connection.take() {
            drop(old);
            let connection = DoIpTcpConnection::connect_doip_tcp(
                self.local_addr,
                self.remote_addr,
                self.la,
                self.timings.clone(),
            )
            .await?;
            self.notify_new_cnx = true;
            self.notify_doip_routed = true;
            self.connection = Some(connection);
        }
        Ok(())
    }
}

async fn doip_scenario_receive(cnx: &mut DoIpConnection) -> Result<ScenarioMessage, ScenarioError> {
    let buffer_holder = &mut cnx.receive_buffer_holder;
    loop {
        let msg = cnx
            .connection
            .as_mut()
            .unwrap() // Here connection cannot be None
            .receive_message(|_, size| {
                let mut v = buffer_holder.take().unwrap();
                v.resize(size, 0);
                v
            })
            .await
            .map_err(|_| ScenarioError::NetworkConnectorDead)?;
        match msg {
            doip_rw_tokio::DoIpTcpMessage::AliveCheckRequest(_) => {
                return Ok(ScenarioMessage::AliveCheckReq)
            }
            doip_rw_tokio::DoIpTcpMessage::AliveCheckResponse(_) => {
                return Ok(ScenarioMessage::AliveCheckRsp)
            }
            doip_rw_tokio::DoIpTcpMessage::DiagnosticMessage(diag) => {
                let uds = uds_rw::uds_read(
                    &mut Cursor::new(diag.user_data.get_ref()),
                    diag.user_data.get_ref().len(),
                )?;
                if let UdsBuffer::Owned(v) = diag.user_data {
                    *buffer_holder = Some(v);
                }
                return Ok(ScenarioMessage::Uds(uds));
            }
            _ => {
                continue;
            }
        }
    }
}

async fn send_alive_check_rsp(cnx: &mut DoIpConnection) -> Result<(), ScenarioError> {
    cnx.connection
        .as_mut()
        .unwrap() // Cannot be None
        .send_alive_check_response(cnx.la)
        .await?;
    Ok(())
}

async fn send_uds(
    cnx: &mut DoIpConnection,
    ta: LogicalAddress,
    uds_req: UdsMessage,
) -> Result<(), ScenarioError> {
    let mut uds_bytes: Vec<u8> = vec![];
    uds_write(&mut uds_bytes, &uds_req)
        .map_err(|_| ScenarioError::UnexpectedUdsMessage(uds_req))?;

    let ack = doip_rw_tokio::send_uds(
        cnx.connection.as_mut().unwrap(), // Cannot be None
        ta,
        UdsBuffer::Borrowed(&uds_bytes),
        |_, size| {
            let mut v = cnx.ack_buffer_holder.take().unwrap();
            v.resize(size, 0);
            v
        },
        Duration::from_secs(1),
    )
    .await?;
    match ack {
        doip_rw_tokio::DoIpTcpMessage::DiagnosticMessagePositiveAck(ack) => {
            if let UdsBuffer::Owned(v) = ack.previous_diagnostic_message_data {
                cnx.ack_buffer_holder = Some(v);
            }
        }
        doip_rw_tokio::DoIpTcpMessage::DiagnosticMessageNegativeAck(nack) => {
            if let UdsBuffer::Owned(v) = nack.previous_diagnostic_message_data {
                cnx.ack_buffer_holder = Some(v);
            }
        }
        _ => {}
    }
    Ok(())
}

impl From<DoIpCnxError> for ScenarioError {
    fn from(value: DoIpCnxError) -> Self {
        match value {
            DoIpCnxError::ConnectionError(_) => Self::NetworkConnectorDead,
            DoIpCnxError::ConnectionTimeout => Self::NetworkConnectorDead,
            DoIpCnxError::InvalidMessageToSend => Self::NetworkConnectorDead,
            DoIpCnxError::InvalidMessageReceived => Self::NetworkConnectorDead,
            DoIpCnxError::RoutingActivationFailed => Self::RoutingActivationFailed,
            DoIpCnxError::SendTimeout => Self::Io(io::Error::from(io::ErrorKind::TimedOut)),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use tokio::{io::AsyncWriteExt, net::TcpSocket};

    #[tokio::test]
    async fn test_discon() {
        let f1 = tokio::spawn(async move {
            let serv = TcpSocket::new_v4().unwrap();
            serv.bind("127.0.0.1:9117".parse().unwrap()).unwrap();
            let (mut tcp, _) = serv.listen(1).unwrap().accept().await.unwrap();
            println!("Accepted incoming call, shuting down");
            let _ = std::io::stdout().flush();
            let _ = tcp.shutdown().await;
        });
        let f2 = tokio::spawn(async move {
            let sock = TcpSocket::new_v4().unwrap();
            let client = sock
                .connect("127.0.0.1:9117".parse().unwrap())
                .await
                .unwrap();
            let buf = [0u8; 4];
            let (_, mut writer) = client.into_split();
            for _ in 1..200 {
                //println!("Writing to socket");
                let _ = writer.write(&buf).await.unwrap();
            }
        });
        let _ = f2.await;
        let _ = f1.await;
    }
}
