use super::common;

#[tokio::test(flavor = "current_thread")]
async fn all_references() {
    let file =
        std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/scenario/reference/references_all.yaml";
    let _res = common::run_test_scenario_file(&file).await;
}
