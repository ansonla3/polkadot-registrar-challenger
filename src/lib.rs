#![recursion_limit = "512"]

#[macro_use]
extern crate log;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate failure;

use adapters::MatrixClient;
use comms::CommsVerifier;
use connector::Connector;
use db::Database;
use identity::IdentityManager;
use primitives::{AccountType, Result};
use std::process::exit;
use tokio::time::{self, Duration};

pub mod adapters;
mod comms;
pub mod connector;
mod db;
mod identity;
mod primitives;
mod verifier;

#[derive(Deserialize)]
pub struct Config {
    pub registrar_db_path: String,
    pub matrix_db_path: String,
    pub log_level: log::LevelFilter,
    pub watcher_url: String,
    pub enable_watcher: bool,
    pub matrix_homeserver: String,
    pub matrix_username: String,
    pub matrix_password: String,
}

pub async fn block() {
    let mut interval = time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
    }
}

pub async fn run(config: Config) -> Result<()> {
    setup(config).await.map(|_| ())
}

pub async fn run_with_feeder(config: Config) -> Result<CommsVerifier> {
    setup(config).await
}

pub async fn setup(config: Config) -> Result<CommsVerifier> {
    info!("Setting up database and manager");
    let db = Database::new(&config.registrar_db_path)?;
    let mut manager = IdentityManager::new(db)?;

    info!("Setting up communication channels");
    let c_connector = manager.register_comms(AccountType::ReservedConnector);
    let c_emitter = manager.register_comms(AccountType::ReservedEmitter);
    let c_matrix = manager.register_comms(AccountType::Matrix);
    let c_feeder = manager.register_comms(AccountType::ReservedFeeder);

    info!("Trying to connect to Watcher");
    let mut counter = 0;
    let mut interval = time::interval(Duration::from_secs(5));

    let mut connector = None;
    loop {
        interval.tick().await;

        // Only connect to Watcher if the config specifies so.
        if !config.enable_watcher {
            break;
        }

        if let Ok(con) = Connector::new(&config.watcher_url, c_connector.clone()).await {
            info!("Connecting to Watcher succeeded");
            connector = Some(con);
            break;
        } else {
            warn!("Connecting to Watcher failed, trying again...");
        }

        if counter == 2 {
            error!("Failed connecting to Watcher, exiting...");
            exit(1);
        }

        counter += 1;
    }

    info!("Starting manager task");
    tokio::spawn(async move {
        manager.start().await;
    });

    info!("Setting up Matrix client");
    let matrix = MatrixClient::new(
        &config.matrix_homeserver,
        &config.matrix_username,
        &config.matrix_password,
        &config.matrix_db_path,
        c_matrix,
        c_emitter,
    )
    .await?;

    info!("Starting Matrix task");
    tokio::spawn(async move {
        matrix.start().await;
    });

    if config.enable_watcher {
        info!("Starting Watcher connector task, listening...");
        tokio::spawn(async move {
            connector.unwrap().start().await;
        });
    } else {
        warn!("Watcher connector task is disabled. Cannot process any requests...");
    }

    Ok(c_feeder)
}
