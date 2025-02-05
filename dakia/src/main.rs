mod config;
mod error;
mod gateway;
mod proxy;
mod qe;
mod shared;

use std::{
    mem::take,
    sync::{Arc, Mutex},
};

use clap::Parser;
use config::{DakiaArgs, DakiaConfig};
use error::DakiaError;
use gateway::HttpGateway;
use pingora::server::{configuration::ServerConf, Server};
use shared::common::get_dakia_ascii_art;
use shared::config_store;

use proxy::http::Proxy;
use shared::into::IntoRef;
use tokio::runtime::Builder;

fn main() {
    println!("{}", get_dakia_ascii_art());

    let dakia_args = DakiaArgs::parse();
    process_args(&dakia_args).unwrap();

    let dakia_config = DakiaConfig::from_args(dakia_args.clone()).unwrap();

    // perform init steps
    init();

    let runtime = Builder::new_current_thread()
        .build()
        // if there is any error, just panic
        .unwrap();

    // TODO: add support for TCP, WebSocket and gRPC gateway
    let gateways: Arc<Mutex<Vec<HttpGateway>>> = Arc::new(Mutex::new(vec![]));

    // clone data for passing to the tokio runtime
    let gateways_cloned = gateways.clone();
    let dakia_config_cloned = dakia_config.clone();

    let handle = runtime.spawn(async move {
        let _ = config_store::store(dakia_config_cloned.clone()).await;

        for gateway_config in &dakia_config_cloned.gateways {
            let server_conf: ServerConf = dakia_config_cloned.into_ref();
            let gateway = gateway::build_http(gateway_config, &Arc::new(server_conf))
                .await
                .unwrap();

            // rust mutex guard does not work properly across tokio await, so creating lock guard after await in each loop
            let mut gateway_vector_guard = gateways_cloned.lock().unwrap();
            gateway_vector_guard.push(gateway);
        }
    });

    runtime.block_on(handle).unwrap();

    // we no longer this runtime, pingora runtime will be used instead
    runtime.shutdown_background();

    let mut server = Server::new_with_opt_and_conf(
        dakia_config.to_pingore_opt(&dakia_args),
        dakia_config.into_ref(),
    );
    server.bootstrap();

    let mut gateway_vector_guard = gateways.lock().unwrap();

    // take ownership of vector to pass owned value inside add_service
    let proxy_vector = take(&mut *gateway_vector_guard);

    for gateway in proxy_vector.into_iter() {
        server.add_service(gateway);
    }

    server.run_forever();
}

fn init() {
    env_logger::init();
}

fn process_args(_args: &DakiaArgs) -> Result<(), Box<DakiaError>> {
    if _args.version {
        // version will be printed along with dakia art in the very beginning, so just exist from here
        shared::common::exit();
    }

    if _args.reload {
        // not implemented
        shared::common::exit();
    }

    if _args.debug {
        // not implemented
        shared::common::exit();
    }

    if _args.test {
        // not implemented
        shared::common::exit();
    }

    Ok(())
}
