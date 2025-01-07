use super::error::ScenarioError;
use super::parser;
use doip_rw::LogicalAddress;
use std::net::SocketAddr;
use tokio::sync::mpsc;

use super::doip_ops;

pub async fn scenario(
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
    la: LogicalAddress,
    ta: LogicalAddress,
    steps: parser::Steps,
) -> Result<(), ScenarioError> {
    let (req_tx, mut req_rx) = mpsc::channel(1);
    let (rsp_tx, rsp_rx) = mpsc::channel(3); // 2 because the Notifications can come in burst

    let mut doip_cnx = doip_ops::DoIpConnection::connect(local_addr, remote_addr, la).await?;

    tokio::spawn(async move {
        loop {
            #[rustfmt::skip]
            tokio::select! {
                rscen = doip_cnx.recv_scenario() => {
                    match rscen {
			Ok(scen) => {
                            if rsp_tx.send(scen).await.is_err() {
                                break;
                            }
			},
			Err(ScenarioError::Io(_)) => {
			    println!("DoIp connection lost, trying to reconnect");
			    let _ = doip_cnx.reconnect().await;
			},
			Err(e) => {
			    println!("Scenario encountered error {}, reconnecting DoIp anyway", e);
			    let _ = doip_cnx.reconnect().await;
			},
                    }
                },
                Some(scen_req) = req_rx.recv() => {
                    if doip_cnx.send_scenario(ta, scen_req)
                        .await
                        .is_err()
                    {
                        break;
                    }
                },
            };
        }
    });

    super::executor::execute(steps, req_tx, rsp_rx).await?;
    Ok(())
}
