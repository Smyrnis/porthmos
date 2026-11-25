#![cfg(unix)]

use porthmos_scp::Scp;
use porthmos_ssh::testing::{self, SshServer};
use porthmos_vfs::{Answer, Protocol};

#[tokio::test]
async fn an_scp_file_system_offers_a_runner_that_runs_commands() {
    let server = SshServer::start(Default::default()).await;
    let fs = Scp::default()
        .with_connect_options(server.connect_options())
        .connect(&server.target(), &mut testing::answers(vec![Some(Answer::Confirmed)]))
        .await
        .unwrap();

    let output = fs.runner().expect("scp runs commands").run("echo hi").await.unwrap();

    assert_eq!(output.stdout, b"hi\n");
}
