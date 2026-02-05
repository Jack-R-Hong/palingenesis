pub async fn handle_logs(follow: bool, tail: u32, since: Option<String>) -> anyhow::Result<()> {
    if follow {
        println!("logs follow not implemented (Story 1.11)");
    } else {
        println!("logs not implemented (Story 1.11)");
    }

    if tail != 20 {
        println!("logs tail set to {tail}");
    }

    if let Some(since) = since {
        println!("logs since {since} not implemented (Story 1.11)");
    }

    Ok(())
}
