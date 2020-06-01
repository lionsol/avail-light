//! Contains code required to plug the networking together with the rest of the service.
//!
//! Contrary to the [crate::network] module, this module is aware of the other tasks of the
//! service.

use crate::network;

use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use hashbrown::HashMap;

/// Message that can be sent to the network task by the other parts of the code.
pub enum ToNetwork {
    BlocksRequest(
        network::BlocksRequestConfig,
        oneshot::Sender<Result<Vec<network::BlockData>, ()>>,
    ),
}

/// Configuration for that task.
pub struct Config {
    /// Prototype for the network worker.
    pub network_builder: network::builder::NetworkBuilder,
    /// Sender that reports messages to the outside of the service.
    pub to_service_out: mpsc::Sender<super::Event>,
    /// Receiver to receive messages that the networking task will process.
    pub to_network: mpsc::Receiver<super::network_task::ToNetwork>,
}

/// Runs the task.
pub async fn run_networking_task(mut config: Config) {
    let mut network = config.network_builder.build().await;

    // Associates network-assigned block request ids to senders.
    let mut pending_blocks_requests = HashMap::<_, oneshot::Sender<_>>::new();

    loop {
        futures::select! {
            ev = network.next_event().fuse() => {
                match ev {
                    network::Event::BlockAnnounce(_) => {},
                    network::Event::BlocksRequestFinished { id, result } => {
                        let sender = pending_blocks_requests.remove(&id).unwrap();
                        let _ = sender.send(result);
                    }
                }
            }
            ev = config.to_network.next() => {
                match ev {
                    None => return,
                    Some(ToNetwork::BlocksRequest(rq, send_back)) => {
                        let id = network.start_block_request(rq).await;
                        pending_blocks_requests.insert(id, send_back);
                    }
                }
            }
        }
    }
}
