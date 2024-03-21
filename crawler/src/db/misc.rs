use super::client::DbClient;

// Helper fn to update both status and security_txt_status
pub async fn update_program_status_and_security_txt_status(
    db: &DbClient,
    pubkey: &str,
    has_succeeded: bool,
    has_security_txt: bool,
    is_closed: bool,
) {
    db.update_security_txt_status(pubkey, has_security_txt)
        .await
        .unwrap();
    db.update_program_status(pubkey, has_succeeded)
        .await
        .unwrap();
    db.set_is_closed(pubkey, is_closed).await.unwrap();
}
