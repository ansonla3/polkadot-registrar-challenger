use super::db_path;
use super::mocks::*;
use crate::{run, Database2};
use matrix_sdk::identifiers::UserId;
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::convert::TryFrom;

#[test]
fn verify_matrix() {
    let mut rt = Runtime::new().unwrap();
    rt.block_on(async {
        let db = Database2::new(&db_path()).unwrap();
        let manager = Arc::new(EventManager2::new());
        let (_, matrix_child) = manager.child();

        let my_user_id = UserId::try_from("@registrar:matrix.org").unwrap();
        let matrix_transport = MatrixMocker::new(matrix_child, my_user_id);

        let matrix_comms = run::<ConnectorMocker, _, _, _, _, _, _>(
            true,
            Arc::clone(&manager),
            db,
            matrix_transport,
            DummyTransport::new(),
            DummyTransport::new(),
        ).await.unwrap();
    });
}