// Temporary: downloads one song with the Mac session, to compare with iOS.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let dir = std::env::var_os("PROBE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| dirs::data_dir().unwrap().join("aural"));
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("session.json"))?)?;
    let api = ytmusic::YtMusic::with_cookies(saved["cookies"].as_str().unwrap().to_owned())
        .as_user(saved["authuser"].as_u64().unwrap_or(0) as _);
    let id = std::env::args().nth(1).unwrap_or("cb0NxdA2z3c".into());
    let (_format, mut audio) = api.open_audio(&id).await?;
    let mut got = 0;
    while let Some(chunk) = audio.chunk().await? {
        got += chunk.len();
    }
    println!("downloaded {got} bytes of {:?}", audio.total());
    Ok(())
}
