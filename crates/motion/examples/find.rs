//! `cargo run -p motion --example find -- "<title>" "<artist>" [seconds] [album]`: looks the album's
//! motion artwork up and downloads the 486 px loop to the system temp folder.

use std::time::Duration;

use motion::{MotionQuery, MotionSearch};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let title = args.next().unwrap_or_default();
    let artist = args.next().unwrap_or_default();
    let duration = args
        .next()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs);
    let album = args.next();
    let search = MotionSearch::default();
    let Some(art) = search
        .find(&MotionQuery::new(
            &title,
            &artist,
            album.as_deref(),
            duration,
        ))
        .await?
    else {
        println!("no motion artwork");
        return Ok(());
    };
    println!("album {} → {}", art.album_id, art.master);
    let found = search.fetch(&art, 486).await?;
    let path = std::env::temp_dir().join("aural-motion.mp4");
    std::fs::write(&path, &found.bytes)?;
    println!(
        "{} px, {} KiB → {}",
        found.side,
        found.bytes.len() / 1024,
        path.display()
    );
    Ok(())
}
